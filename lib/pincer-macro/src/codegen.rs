//! Code generation for pincer proc-macros.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Ident, Type, Visibility};

use crate::attrs::{CollectionFormat, MethodParam, ParamKind};

/// Generate the client struct and builder for a trait-based API.
pub fn generate_client_struct(
    vis: &Visibility,
    client_name: &Ident,
    builder_name: &Ident,
    base_url: &str,
) -> TokenStream {
    quote! {
        /// Generated client struct implementing the API trait.
        #vis struct #client_name {
            client: ::pincer::HyperClient,
            base_url: ::pincer::url::Url,
        }

        impl #client_name {
            /// Get the base URL.
            #[must_use]
            pub fn base_url(&self) -> &::pincer::url::Url {
                &self.base_url
            }
        }

        /// Builder for the client struct.
        #vis struct #builder_name {
            base_url: Option<String>,
            client: Option<::pincer::HyperClient>,
            client_builder: ::pincer::HyperClientBuilder,
        }

        impl Default for #builder_name {
            fn default() -> Self {
                Self {
                    base_url: None,
                    client: None,
                    client_builder: ::pincer::HyperClient::builder(),
                }
            }
        }

        impl #builder_name {
            /// Set a custom base URL (default: #base_url).
            #[must_use]
            pub fn base_url(mut self, url: impl Into<String>) -> Self {
                self.base_url = Some(url.into());
                self
            }

            /// Set a custom HTTP client.
            ///
            /// Note: This replaces any middleware configured via `configure_client` or helper methods.
            #[must_use]
            pub fn client(mut self, client: ::pincer::HyperClient) -> Self {
                self.client = Some(client);
                self
            }

            /// Configure the underlying HTTP client builder.
            ///
            /// # Example
            ///
            /// ```ignore
            /// let client = MyApi::client()
            ///     .configure_client(|b| b.with_retry(3).with_logging())
            ///     .build()?;
            /// ```
            #[must_use]
            pub fn configure_client<F>(mut self, f: F) -> Self
            where
                F: FnOnce(::pincer::HyperClientBuilder) -> ::pincer::HyperClientBuilder,
            {
                self.client_builder = f(self.client_builder);
                self
            }

            /// Build the client.
            pub fn build(self) -> ::pincer::Result<#client_name> {
                let base_url = self.base_url.unwrap_or_else(|| #base_url.to_string());
                let base_url = ::pincer::url::Url::parse(&base_url)
                    .map_err(::pincer::Error::InvalidUrl)?;

                // If a custom client was provided, use it; otherwise build from the builder
                let client = self.client.unwrap_or_else(|| self.client_builder.build());

                Ok(#client_name { client, base_url })
            }
        }
    }
}

/// Generate URL building code with path parameter substitution.
///
/// Uses percent-encoding to properly encode path parameter values,
/// ensuring special characters like spaces, `&`, `?`, `/` are handled correctly.
pub fn generate_url_code(path_template: &str, params: &[MethodParam]) -> TokenStream {
    // Find path parameters and generate substitutions
    let path_params: Vec<_> = params
        .iter()
        .filter_map(|p| match &p.kind {
            ParamKind::Path(alias) => {
                let name = &p.name;
                let param_name = p.name.to_string();
                let key = alias.as_deref().unwrap_or(&param_name);
                Some((key.to_string(), name.clone()))
            }
            _ => None,
        })
        .collect();

    // Build the path with substitutions using percent-encoding
    // Use PATH_SEGMENT encoding which allows unreserved characters (- _ . ~)
    // but encodes special chars like spaces, &, ?, /, etc.
    let mut path_expr = quote! {
        // Define the path segment encoding set (encodes all except unreserved + sub-delims)
        // Unreserved: A-Z a-z 0-9 - . _ ~
        const PATH_SEGMENT_ENCODE_SET: &::pincer::percent_encoding::AsciiSet =
            &::pincer::percent_encoding::CONTROLS
                .add(b' ')
                .add(b'"')
                .add(b'#')
                .add(b'<')
                .add(b'>')
                .add(b'`')
                .add(b'?')
                .add(b'{')
                .add(b'}')
                .add(b'/')
                .add(b'\\')
                .add(b'%');
        let mut path = #path_template.to_string();
    };

    for (key, name) in &path_params {
        let placeholder = format!("{{{key}}}");
        path_expr = quote! {
            #path_expr
            path = path.replace(
                #placeholder,
                &::pincer::percent_encoding::utf8_percent_encode(
                    &#name.to_string(),
                    PATH_SEGMENT_ENCODE_SET,
                ).to_string()
            );
        };
    }

    quote! {
        #path_expr
        let url = self.base_url.join(&path)
            .map_err(::pincer::Error::InvalidUrl)?;
    }
}

/// Generate query parameter code.
///
/// Supports:
/// - Simple types: `#[query] page: u32` → `?page=1`
/// - Optional types: `#[query] page: Option<u32>` → skipped if None
/// - Vec types: `#[query] tags: Vec<String>` → `?tags=a&tags=b` (multi format, default)
/// - Vec types with format: `#[query(format = "csv")] tags: Vec<String>` → `?tags=a,b,c`
/// - Struct types: `#[query] params: SearchParams` → serialized via `serde_html_form`
pub fn generate_query_code(params: &[MethodParam]) -> TokenStream {
    let query_params: Vec<_> = params
        .iter()
        .filter_map(|p| match &p.kind {
            ParamKind::Query(options) => {
                let name = &p.name;
                let param_name = p.name.to_string();
                let key = options.alias.as_deref().unwrap_or(&param_name).to_string();
                Some((key, name.clone(), p.ty.clone(), options.format))
            }
            _ => None,
        })
        .collect();

    if query_params.is_empty() {
        return quote! {};
    }

    let mut append_statements = Vec::new();

    for (key, name, ty, format) in &query_params {
        if is_option_type(ty) {
            // Option<T>: skip if None
            append_statements.push(quote! {
                if let Some(value) = #name {
                    query.append_pair(#key, &value.to_string());
                }
            });
        } else if is_vec_type(ty) {
            // Vec<T>: use the specified collection format
            append_statements.push(generate_vec_query_code(key, name, *format));
        } else if is_struct_type(ty) {
            // Struct type: use ToQueryPairs trait (requires #[derive(Query)] on the struct)
            append_statements.push(quote! {
                for (key, value) in ::pincer::ToQueryPairs::to_query_pairs(#name) {
                    query.append_pair(&key, &value);
                }
            });
        } else {
            // Simple type
            append_statements.push(quote! {
                query.append_pair(#key, &#name.to_string());
            });
        }
    }

    quote! {
        let mut url = url;
        {
            let mut query = url.query_pairs_mut();
            #(#append_statements)*
        }
    }
}

/// Generate code for serializing a Vec<T> query parameter with the given format.
fn generate_vec_query_code(key: &str, name: &Ident, format: CollectionFormat) -> TokenStream {
    match format.separator() {
        None => {
            // Multi format: repeated parameters ?tags=a&tags=b&tags=c
            quote! {
                for item in #name {
                    query.append_pair(#key, &item.to_string());
                }
            }
        }
        Some(sep) => {
            // Separated format: ?tags=a,b,c or ?tags=a|b|c etc.
            quote! {
                if !#name.is_empty() {
                    let value = #name.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(#sep);
                    query.append_pair(#key, &value);
                }
            }
        }
    }
}

/// Generate headers code.
///
/// Supports:
/// - Static headers (User-Agent, Accept)
/// - Trait-level headers (from `#[headers(...)]` on the trait)
/// - Single header: `#[header("Authorization")] token: &str`
/// - Header map: `#[headers] extra: HashMap<String, String>`
pub fn generate_headers_code(
    params: &[MethodParam],
    user_agent: &str,
    trait_headers: &[(String, String)],
) -> TokenStream {
    let mut headers = quote! {
        .header("User-Agent", #user_agent)
        .header("Accept", "application/json")
    };

    // Add trait-level headers
    for (key, value) in trait_headers {
        headers = quote! {
            #headers
            .header(#key, #value)
        };
    }

    // First add individual headers
    for param in params {
        if let ParamKind::Header(header_name) = &param.kind {
            let name = &param.name;
            headers = quote! {
                #headers
                .header(#header_name, #name)
            };
        }
    }

    // Then add header maps
    for param in params {
        if matches!(param.kind, ParamKind::Headers) {
            let name = &param.name;
            headers = quote! {
                #headers
                .headers(#name.into_iter().map(|(k, v)| (k.to_string(), v.to_string())))
            };
        }
    }

    headers
}

/// Check if the method has multipart parameters.
pub fn has_multipart_params(params: &[MethodParam]) -> bool {
    params
        .iter()
        .any(|p| matches!(&p.kind, ParamKind::Multipart(_)))
}

/// Generate pre-body code that runs before the request builder.
/// This is used for multipart forms that need to build data first.
pub fn generate_pre_body_code(params: &[MethodParam]) -> TokenStream {
    let multipart_params: Vec<_> = params
        .iter()
        .filter_map(|p| match &p.kind {
            ParamKind::Multipart(options) => Some((p, options)),
            _ => None,
        })
        .collect();

    if multipart_params.is_empty() {
        return quote! {};
    }

    generate_multipart_pre_body_code(&multipart_params)
}

/// Generate multipart form building code.
fn generate_multipart_pre_body_code(
    params: &[(&MethodParam, &crate::attrs::MultipartOptions)],
) -> TokenStream {
    let mut form_parts = Vec::new();

    for (param, options) in params {
        let name = &param.name;
        let field_name = options.name.as_ref().map_or_else(
            || {
                let param_name = param.name.to_string();
                quote! { #param_name }
            },
            |n| quote! { #n },
        );

        // Check if the parameter type is Vec<Part>
        if is_vec_type(&param.ty) {
            // For Vec<Part>, iterate and add each part with a modified name
            form_parts.push(quote! {
                for (i, part) in #name.into_iter().enumerate() {
                    let part_name = format!("{}[{}]", #field_name, i);
                    let named_part = ::pincer::Part::new(part_name, part.data().clone())
                        .with_content_type(part.content_type().unwrap_or("application/octet-stream"));
                    let named_part = if let Some(filename) = part.filename() {
                        named_part.with_filename(filename)
                    } else {
                        named_part
                    };
                    __multipart_form = __multipart_form.part(named_part);
                }
            });
        } else {
            // Single Part - set the name from the attribute or param name
            form_parts.push(quote! {
                {
                    let part = &#name;
                    let named_part = ::pincer::Part::new(#field_name, part.data().clone())
                        .with_content_type(part.content_type().unwrap_or("application/octet-stream"));
                    let named_part = if let Some(filename) = part.filename() {
                        named_part.with_filename(filename)
                    } else {
                        named_part
                    };
                    __multipart_form = __multipart_form.part(named_part);
                }
            });
        }
    }

    quote! {
        let mut __multipart_form = ::pincer::Form::new();
        #(#form_parts)*
        let (__multipart_content_type, __multipart_body) = __multipart_form.into_body();
    }
}

/// Generate body code.
pub fn generate_body_code(params: &[MethodParam]) -> TokenStream {
    // Check for multipart params first (they take precedence)
    if has_multipart_params(params) {
        return quote! {
            .header("content-type".to_string(), __multipart_content_type)
            .body(__multipart_body)
        };
    }

    // Check for other body types
    for param in params {
        match &param.kind {
            ParamKind::Body => {
                let name = &param.name;
                return quote! { .json(#name)? };
            }
            ParamKind::Form => {
                let name = &param.name;
                return quote! { .form(#name)? };
            }
            _ => {}
        }
    }

    quote! {}
}

/// Check if a type is `Option<T>`.
fn is_option_type(ty: &Type) -> bool {
    matches!(ty, Type::Path(type_path) if type_path.path.segments.last().is_some_and(|seg| seg.ident == "Option"))
}

/// The kind of return type for a method.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReturnTypeKind {
    /// JSON deserialization (default): `Result<T>`
    Json,
    /// Raw response: `Result<Response<Bytes>>`
    RawResponse,
    /// Unit type: `Result<()>`
    Unit,
}

/// Analyze the return type to determine how to handle the response.
///
/// Extracts the inner type from `Result<T>` or `Result<Option<T>>` and determines:
/// - `RawResponse`: If the type is `Response<_>` or `Response<Bytes>`
/// - `Unit`: If the type is `()`
/// - `Json`: Everything else (default - deserialize JSON)
pub fn analyze_return_type(return_type: &syn::ReturnType) -> ReturnTypeKind {
    let ty = match return_type {
        syn::ReturnType::Default => return ReturnTypeKind::Unit,
        syn::ReturnType::Type(_, ty) => ty.as_ref(),
    };

    // Unwrap Result<T> to get T
    let inner = unwrap_result_type(ty).unwrap_or(ty);

    // Unwrap Option<T> if present (for not_found_as_none)
    let inner = unwrap_option_type(inner).unwrap_or(inner);

    // Check for unit type ()
    if is_unit_type(inner) {
        return ReturnTypeKind::Unit;
    }

    // Check for Response<_> type
    if is_response_type(inner) {
        return ReturnTypeKind::RawResponse;
    }

    ReturnTypeKind::Json
}

/// Check if a type is the unit type `()`.
pub fn is_unit_type(ty: &Type) -> bool {
    matches!(ty, Type::Tuple(tuple) if tuple.elems.is_empty())
}

/// Check if a type is `Response<_>` (pincer response type).
fn is_response_type(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty
        && let Some(segment) = type_path.path.segments.last()
    {
        return segment.ident == "Response";
    }
    false
}

/// Unwrap `Result<T>` to get `T`, returns None if not a Result.
fn unwrap_result_type(ty: &Type) -> Option<&Type> {
    if let Type::Path(type_path) = ty
        && let Some(segment) = type_path.path.segments.last()
        && segment.ident == "Result"
        && let syn::PathArguments::AngleBracketed(args) = &segment.arguments
        && let Some(syn::GenericArgument::Type(inner)) = args.args.first()
    {
        return Some(inner);
    }
    None
}

/// Unwrap `Option<T>` to get `T`, returns None if not an Option.
fn unwrap_option_type(ty: &Type) -> Option<&Type> {
    if let Type::Path(type_path) = ty
        && let Some(segment) = type_path.path.segments.last()
        && segment.ident == "Option"
        && let syn::PathArguments::AngleBracketed(args) = &segment.arguments
        && let Some(syn::GenericArgument::Type(inner)) = args.args.first()
    {
        return Some(inner);
    }
    None
}

/// Check if a type is `Vec<T>`.
fn is_vec_type(ty: &Type) -> bool {
    matches!(ty, Type::Path(type_path) if type_path.path.segments.last().is_some_and(|seg| seg.ident == "Vec"))
}

/// Generate a generic wrapper struct for the wrapper mode.
///
/// This generates a struct that wraps any `PincerClient` implementation:
/// ```ignore
/// pub struct GitHubApiClient<C> {
///     client: C,
///     base_url: Url,
/// }
/// ```
pub fn generate_wrapper_struct(
    vis: &Visibility,
    client_name: &Ident,
    base_url: &str,
) -> TokenStream {
    quote! {
        /// Generated generic client wrapper implementing the API trait.
        ///
        /// Wraps any `PincerClient` implementation to provide the API.
        #vis struct #client_name<C> {
            client: C,
            base_url: ::pincer::url::Url,
        }

        impl<C> #client_name<C> {
            /// Create a new client wrapper with the default base URL.
            #[must_use]
            pub fn new(client: C) -> Self {
                Self {
                    client,
                    base_url: ::pincer::url::Url::parse(#base_url)
                        .expect("invalid base URL in macro"),
                }
            }

            /// Create a new client wrapper with a custom base URL.
            #[must_use]
            pub fn with_base_url(client: C, base_url: ::pincer::url::Url) -> Self {
                Self { client, base_url }
            }

            /// Get the base URL.
            #[must_use]
            pub fn base_url(&self) -> &::pincer::url::Url {
                &self.base_url
            }

            /// Get a reference to the inner client.
            #[must_use]
            pub fn inner(&self) -> &C {
                &self.client
            }

            /// Consume the wrapper and return the inner client.
            #[must_use]
            pub fn into_inner(self) -> C {
                self.client
            }
        }

        impl<C: Clone> Clone for #client_name<C> {
            fn clone(&self) -> Self {
                Self {
                    client: self.client.clone(),
                    base_url: self.base_url.clone(),
                }
            }
        }

        impl<C: ::pincer::PincerClient> ::pincer::PincerClient for #client_name<C> {
            fn execute(
                &self,
                request: ::pincer::Request<::bytes::Bytes>,
            ) -> impl ::std::future::Future<Output = ::pincer::Result<::pincer::Response<::bytes::Bytes>>> + Send {
                self.client.execute(request)
            }

            fn base_url(&self) -> &::pincer::url::Url {
                &self.base_url
            }
        }
    }
}

/// Check if a type appears to be a struct (not a primitive, Option, or Vec).
///
/// This is a heuristic: if the type starts with an uppercase letter and isn't
/// a known generic type (Option, Vec, String, etc.), we assume it's a struct
/// that implements Serialize.
fn is_struct_type(ty: &Type) -> bool {
    // Handle references: &T or &mut T
    let inner_ty = match ty {
        Type::Reference(type_ref) => &*type_ref.elem,
        other => other,
    };

    if let Type::Path(type_path) = inner_ty
        && let Some(segment) = type_path.path.segments.last()
    {
        let name = segment.ident.to_string();
        // Known primitive/standard types that should NOT be treated as structs
        let non_struct_types = [
            "bool", "char", "str", "String", "i8", "i16", "i32", "i64", "i128", "isize", "u8",
            "u16", "u32", "u64", "u128", "usize", "f32", "f64", "Option", "Vec", "Box", "Result",
            "Cow",
        ];
        return !non_struct_types.contains(&name.as_str());
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_option_type() {
        let ty: Type = syn::parse_quote!(Option<String>);
        assert!(is_option_type(&ty));

        let ty: Type = syn::parse_quote!(String);
        assert!(!is_option_type(&ty));
    }

    #[test]
    fn test_is_vec_type() {
        let ty: Type = syn::parse_quote!(Vec<String>);
        assert!(is_vec_type(&ty));

        let ty: Type = syn::parse_quote!(Vec<u32>);
        assert!(is_vec_type(&ty));

        let ty: Type = syn::parse_quote!(String);
        assert!(!is_vec_type(&ty));

        let ty: Type = syn::parse_quote!(Option<Vec<String>>);
        assert!(!is_vec_type(&ty)); // Option, not Vec
    }

    #[test]
    fn test_is_struct_type() {
        // Custom struct types
        let ty: Type = syn::parse_quote!(SearchParams);
        assert!(is_struct_type(&ty));

        let ty: Type = syn::parse_quote!(UserFilter);
        assert!(is_struct_type(&ty));

        // References to structs should also be detected
        let ty: Type = syn::parse_quote!(&SearchParams);
        assert!(is_struct_type(&ty));

        let ty: Type = syn::parse_quote!(&mut UserFilter);
        assert!(is_struct_type(&ty));

        // Primitives should NOT be treated as structs
        let ty: Type = syn::parse_quote!(u32);
        assert!(!is_struct_type(&ty));

        let ty: Type = syn::parse_quote!(String);
        assert!(!is_struct_type(&ty));

        let ty: Type = syn::parse_quote!(bool);
        assert!(!is_struct_type(&ty));

        // References to primitives should NOT be structs
        let ty: Type = syn::parse_quote!(&str);
        assert!(!is_struct_type(&ty));

        let ty: Type = syn::parse_quote!(&String);
        assert!(!is_struct_type(&ty));

        // Generic types should NOT be treated as structs
        let ty: Type = syn::parse_quote!(Option<u32>);
        assert!(!is_struct_type(&ty));

        let ty: Type = syn::parse_quote!(Vec<String>);
        assert!(!is_struct_type(&ty));
    }
}
