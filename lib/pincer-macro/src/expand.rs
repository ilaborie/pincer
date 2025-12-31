//! Macro expansion logic for pincer.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{FnArg, Ident, ItemTrait, Pat, TraitItem, TraitItemFn, parse2};

use crate::attrs::{
    HttpMethod, MethodAttrs, MethodOptions, MethodParam, ParamKind, PincerMode,
    extract_path_placeholders, parse_method_options, parse_param_attr, parse_trait_headers,
};
use crate::codegen::{
    ReturnTypeKind, analyze_return_type, generate_body_code, generate_client_struct,
    generate_headers_code, generate_pre_body_code, generate_query_code, generate_url_code,
    generate_wrapper_struct,
};

/// Default user agent string for pincer clients.
pub const DEFAULT_USER_AGENT: &str = concat!("pincer/", env!("CARGO_PKG_VERSION"));

/// Pincer parameter attribute names that should be stripped from generated code.
const PINCER_PARAM_ATTRS: &[&str] = &[
    "path",
    "query",
    "header",
    "headers",
    "body",
    "form",
    "multipart",
];

/// Arguments for the `#[pincer]` attribute.
#[derive(Default)]
pub struct PincerArgs {
    pub url: Option<String>,
    pub user_agent: Option<String>,
    pub mode: PincerMode,
}

impl PincerArgs {
    /// Get the user agent, defaulting to the library version.
    #[must_use]
    pub fn user_agent(&self) -> &str {
        self.user_agent.as_deref().unwrap_or(DEFAULT_USER_AGENT)
    }
}

/// Parse the pincer attribute arguments.
fn parse_pincer_args(attr: TokenStream) -> syn::Result<PincerArgs> {
    let mut args = PincerArgs::default();

    let parser = syn::meta::parser(|meta| {
        if meta.path.is_ident("url") {
            let value: syn::LitStr = meta.value()?.parse()?;
            args.url = Some(value.value());
            Ok(())
        } else if meta.path.is_ident("user_agent") {
            let value: syn::LitStr = meta.value()?.parse()?;
            args.user_agent = Some(value.value());
            Ok(())
        } else if meta.path.is_ident("mode") {
            let value: syn::LitStr = meta.value()?.parse()?;
            args.mode = PincerMode::parse(&value.value()).ok_or_else(|| {
                meta.error(format!(
                    "unknown mode: \"{}\". Expected: \"full\", \"wrapper\", or \"impl_only\"",
                    value.value()
                ))
            })?;
            Ok(())
        } else {
            Err(meta.error("unsupported pincer attribute"))
        }
    });

    syn::parse::Parser::parse2(parser, attr)?;

    // URL is required for full and wrapper modes, optional for impl_only
    if args.url.is_none() && args.mode != PincerMode::ImplOnly {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "missing `url` attribute (required for full and wrapper modes)",
        ));
    }

    Ok(args)
}

/// Information about a parsed trait method.
pub struct TraitMethodInfo {
    /// The method signature.
    pub sig: syn::Signature,
    /// The HTTP method (GET, POST, etc.).
    pub http_method: HttpMethod,
    /// The URL path template.
    pub path: String,
    /// Parsed parameters.
    pub params: Vec<MethodParam>,
    /// Documentation attributes.
    pub docs: Vec<syn::Attribute>,
    /// Method-level options (`not_found_as_none`, timeout, etc.).
    pub options: MethodOptions,
}

/// Expand the `#[pincer]` attribute on a trait.
pub fn expand_pincer_trait(attr: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    let trait_def: ItemTrait = parse2(item)?;
    let args = parse_pincer_args(attr)?;

    let trait_name = &trait_def.ident;
    let vis = &trait_def.vis;

    let trait_headers = parse_trait_headers(&trait_def.attrs)?;
    let methods = extract_trait_methods(&trait_def)?;
    let clean_trait = generate_clean_trait(vis, trait_name, &methods, &trait_def);

    match args.mode {
        PincerMode::Full => {
            // Full mode: generate client struct + builder (current behavior)
            let client_name = format_ident!("{}Client", trait_name);
            let builder_name = format_ident!("{}ClientBuilder", trait_name);
            let base_url = args
                .url
                .as_ref()
                .ok_or_else(|| syn::Error::new(trait_name.span(), "URL required for full mode"))?;

            let client_and_builder =
                generate_client_struct(vis, &client_name, &builder_name, base_url);
            let trait_impl =
                generate_trait_impl(trait_name, &client_name, &methods, &args, &trait_headers);

            Ok(quote! {
                #clean_trait
                #client_and_builder
                #trait_impl
            })
        }
        PincerMode::Wrapper => {
            // Wrapper mode: generate generic wrapper struct
            let client_name = format_ident!("{}Client", trait_name);
            let base_url = args.url.as_ref().ok_or_else(|| {
                syn::Error::new(trait_name.span(), "URL required for wrapper mode")
            })?;

            let wrapper_struct = generate_wrapper_struct(vis, &client_name, base_url);
            let wrapper_impl = generate_wrapper_trait_impl(
                trait_name,
                &client_name,
                &methods,
                &args,
                &trait_headers,
            );

            Ok(quote! {
                #clean_trait
                #wrapper_struct
                #wrapper_impl
            })
        }
        PincerMode::ImplOnly => {
            // Impl-only mode: generate blanket impl for PincerClient
            let blanket_impl = generate_blanket_impl(trait_name, &methods, &args, &trait_headers);

            Ok(quote! {
                #clean_trait
                #blanket_impl
            })
        }
    }
}

/// Extract methods from a trait definition.
fn extract_trait_methods(trait_def: &ItemTrait) -> syn::Result<Vec<TraitMethodInfo>> {
    let mut methods = Vec::new();

    for item in &trait_def.items {
        if let TraitItem::Fn(method) = item {
            // Find HTTP method attribute
            let http_attr = find_http_attribute(&method.attrs)?;

            if let Some((http_method, path)) = http_attr {
                let params = parse_trait_method_params(method, &path, http_method)?;
                let docs = method
                    .attrs
                    .iter()
                    .filter(|a| a.path().is_ident("doc"))
                    .cloned()
                    .collect();

                // Parse method-level options (not_found_as_none, timeout, etc.)
                let options = parse_method_options(&method.attrs)?;

                methods.push(TraitMethodInfo {
                    sig: method.sig.clone(),
                    http_method,
                    path,
                    params,
                    docs,
                    options,
                });
            }
        }
    }

    Ok(methods)
}

/// Find and parse HTTP method attribute from a method's attributes.
fn find_http_attribute(attrs: &[syn::Attribute]) -> syn::Result<Option<(HttpMethod, String)>> {
    for attr in attrs {
        let path = attr.path();

        // Check for standard method attributes
        let method = if path.is_ident("get") {
            Some(HttpMethod::Get)
        } else if path.is_ident("post") {
            Some(HttpMethod::Post)
        } else if path.is_ident("put") {
            Some(HttpMethod::Put)
        } else if path.is_ident("delete") {
            Some(HttpMethod::Delete)
        } else if path.is_ident("patch") {
            Some(HttpMethod::Patch)
        } else if path.is_ident("head") {
            Some(HttpMethod::Head)
        } else if path.is_ident("options") {
            Some(HttpMethod::Options)
        } else {
            None
        };

        if let Some(method) = method {
            // Parse the path from the attribute
            let path_str = parse_attr_path(attr)?;
            return Ok(Some((method, path_str)));
        }

        // Check for #[http("METHOD /path")]
        if path.is_ident("http") {
            let spec = parse_attr_path(attr)?;
            let (method_str, url_path) = spec.split_once(' ').ok_or_else(|| {
                syn::Error::new_spanned(
                    attr,
                    "expected format: \"METHOD /path\" (e.g., \"GET /users/{id}\")",
                )
            })?;

            let method = HttpMethod::parse(method_str).ok_or_else(|| {
                syn::Error::new_spanned(
                    attr,
                    format!(
                        "unsupported HTTP method: {method_str}. Supported: GET, POST, PUT, DELETE, PATCH, HEAD, OPTIONS"
                    ),
                )
            })?;

            return Ok(Some((method, url_path.to_string())));
        }
    }

    Ok(None)
}

/// Parse the path string from an attribute.
fn parse_attr_path(attr: &syn::Attribute) -> syn::Result<String> {
    match &attr.meta {
        syn::Meta::List(meta_list) => {
            let str_lit: syn::LitStr = syn::parse2(meta_list.tokens.clone())?;
            Ok(str_lit.value())
        }
        _ => Err(syn::Error::new_spanned(attr, "expected string argument")),
    }
}

/// Parse method parameters from a trait method.
///
/// Parameters are classified as follows:
/// 1. Explicit attributes (`#[path]`, `#[query]`, `#[body]`, etc.) take precedence
/// 2. Parameters matching URL placeholders are auto-classified as Path
/// 3. For body-supporting methods (POST, PUT, PATCH), a single remaining param becomes Body
/// 4. Multiple unclassified params or unclassified params on non-body methods cause errors
fn parse_trait_method_params(
    method: &TraitItemFn,
    path_template: &str,
    http_method: HttpMethod,
) -> syn::Result<Vec<MethodParam>> {
    let placeholders = extract_path_placeholders(path_template);
    let mut params = Vec::new();
    let mut unclassified: Vec<(Ident, syn::Type, &syn::PatType)> = Vec::new();

    for input in &method.sig.inputs {
        if let FnArg::Typed(pat_type) = input {
            // Get the parameter name
            let name = match pat_type.pat.as_ref() {
                Pat::Ident(pat_ident) => pat_ident.ident.clone(),
                _ => continue,
            };

            // Get the parameter type
            let ty = (*pat_type.ty).clone();

            // Check for explicit attribute first
            if let Some(kind) = pat_type.attrs.iter().find_map(parse_param_attr) {
                params.push(MethodParam { name, ty, kind });
                continue;
            }

            // Check if param name matches a URL placeholder
            let param_name_str = name.to_string();
            if placeholders.contains(&param_name_str) {
                params.push(MethodParam {
                    name,
                    ty,
                    kind: ParamKind::Path(None),
                });
                continue;
            }

            // No explicit attribute and no placeholder match -> unclassified
            unclassified.push((name, ty, pat_type));
        }
    }

    // Handle unclassified parameters
    match unclassified.len() {
        0 => {}
        1 if http_method.supports_body() => {
            // Single unclassified param becomes Body for body-supporting methods
            if let Some((name, ty, _)) = unclassified.into_iter().next() {
                params.push(MethodParam {
                    name,
                    ty,
                    kind: ParamKind::Body,
                });
            }
        }
        1 => {
            // Non-body method with unclassified param -> error
            if let Some((name, _, pat_type)) = unclassified.first() {
                return Err(syn::Error::new_spanned(
                    pat_type,
                    format!(
                        "parameter '{}' does not match any URL placeholder (available: {:?}) \
                         and {} requests do not support body. \
                         Add #[query] or another explicit attribute.",
                        name,
                        placeholders,
                        http_method.as_str().to_uppercase()
                    ),
                ));
            }
        }
        _ => {
            // Multiple unclassified params -> error
            let names: Vec<_> = unclassified.iter().map(|(n, _, _)| n.to_string()).collect();
            if let Some((_, _, pat_type)) = unclassified.get(1) {
                return Err(syn::Error::new_spanned(
                    pat_type,
                    format!(
                        "multiple unattributed parameters found: {names:?}. \
                         Only one body parameter is allowed. \
                         Add explicit attributes to disambiguate.",
                    ),
                ));
            }
        }
    }

    Ok(params)
}

/// Generate a clean trait without pincer-specific attributes.
fn generate_clean_trait(
    vis: &syn::Visibility,
    name: &Ident,
    methods: &[TraitMethodInfo],
    original: &ItemTrait,
) -> TokenStream {
    // Copy non-pincer attributes from original trait
    let trait_attrs: Vec<_> = original
        .attrs
        .iter()
        .filter(|a| {
            let path = a.path();
            path.is_ident("doc") || path.is_ident("allow") || path.is_ident("cfg")
        })
        .collect();

    let method_signatures: Vec<_> = methods
        .iter()
        .map(|m| {
            let docs = &m.docs;
            let sig = strip_pincer_attrs_from_sig(&m.sig);
            quote! {
                #(#docs)*
                #sig;
            }
        })
        .collect();

    quote! {
        #(#trait_attrs)*
        #[allow(async_fn_in_trait)]
        #vis trait #name {
            #(#method_signatures)*
        }
    }
}

/// Check if an attribute is a pincer parameter attribute.
fn is_pincer_param_attr(attr: &syn::Attribute) -> bool {
    let path = attr.path();
    PINCER_PARAM_ATTRS.iter().any(|name| path.is_ident(name))
}

/// Strip pincer-specific attributes from a method signature.
fn strip_pincer_attrs_from_sig(sig: &syn::Signature) -> syn::Signature {
    let mut clean_sig = sig.clone();
    clean_sig.inputs = sig
        .inputs
        .iter()
        .map(|arg| match arg {
            FnArg::Typed(pat_type) => {
                let mut clean = pat_type.clone();
                clean.attrs.retain(|attr| !is_pincer_param_attr(attr));
                FnArg::Typed(clean)
            }
            FnArg::Receiver(receiver) => FnArg::Receiver(receiver.clone()),
        })
        .collect();
    clean_sig
}

/// Generate the trait implementation for the client struct.
fn generate_trait_impl(
    trait_name: &Ident,
    client_name: &Ident,
    methods: &[TraitMethodInfo],
    args: &PincerArgs,
    trait_headers: &[(String, String)],
) -> TokenStream {
    let user_agent = args.user_agent();

    let method_impls: Vec<_> = methods
        .iter()
        .map(|m| {
            let sig = strip_pincer_attrs_from_sig(&m.sig);
            let attrs = MethodAttrs::new(m.http_method, m.path.clone());
            let return_type_kind = analyze_return_type(&m.sig.output);
            let method_name = m.sig.ident.to_string();
            let body = generate_method_body(
                &attrs,
                &m.params,
                user_agent,
                &m.options,
                trait_headers,
                return_type_kind,
                &method_name,
            );

            quote! {
                #sig {
                    #body
                }
            }
        })
        .collect();

    quote! {
        impl #trait_name for #client_name {
            #(#method_impls)*
        }
    }
}

/// Generate the trait implementation for a generic wrapper struct.
///
/// This generates code like:
/// ```ignore
/// impl<C: PincerClient> GitHubApi for GitHubApiClient<C> { ... }
/// ```
fn generate_wrapper_trait_impl(
    trait_name: &Ident,
    client_name: &Ident,
    methods: &[TraitMethodInfo],
    args: &PincerArgs,
    trait_headers: &[(String, String)],
) -> TokenStream {
    let user_agent = args.user_agent();

    let method_impls: Vec<_> = methods
        .iter()
        .map(|m| {
            let sig = strip_pincer_attrs_from_sig(&m.sig);
            let attrs = MethodAttrs::new(m.http_method, m.path.clone());
            let return_type_kind = analyze_return_type(&m.sig.output);
            let method_name = m.sig.ident.to_string();
            let body = generate_method_body(
                &attrs,
                &m.params,
                user_agent,
                &m.options,
                trait_headers,
                return_type_kind,
                &method_name,
            );

            quote! {
                #sig {
                    #body
                }
            }
        })
        .collect();

    quote! {
        impl<C: ::pincer::PincerClient> #trait_name for #client_name<C> {
            #(#method_impls)*
        }
    }
}

/// Generate a blanket implementation for any `PincerClient`.
///
/// This generates code like:
/// ```ignore
/// impl<T: PincerClient> GitHubApi for T { ... }
/// ```
fn generate_blanket_impl(
    trait_name: &Ident,
    methods: &[TraitMethodInfo],
    args: &PincerArgs,
    trait_headers: &[(String, String)],
) -> TokenStream {
    let user_agent = args.user_agent();

    let method_impls: Vec<_> = methods
        .iter()
        .map(|m| {
            let sig = strip_pincer_attrs_from_sig(&m.sig);
            let attrs = MethodAttrs::new(m.http_method, m.path.clone());
            let return_type_kind = analyze_return_type(&m.sig.output);
            let method_name = m.sig.ident.to_string();
            let body = generate_blanket_method_body(
                &attrs,
                &m.params,
                user_agent,
                &m.options,
                trait_headers,
                return_type_kind,
                &method_name,
            );

            quote! {
                #sig {
                    #body
                }
            }
        })
        .collect();

    quote! {
        impl<__PincerT: ::pincer::PincerClient> #trait_name for __PincerT {
            #(#method_impls)*
        }
    }
}

/// Generate the body of a method implementation for blanket impls.
///
/// This is similar to `generate_method_body` but uses `PincerClient` trait methods
/// instead of direct field access, enabling blanket implementations for any `T: PincerClient`.
fn generate_blanket_method_body(
    attrs: &MethodAttrs,
    params: &[MethodParam],
    user_agent: &str,
    options: &MethodOptions,
    trait_headers: &[(String, String)],
    return_type_kind: ReturnTypeKind,
    method_name: &str,
) -> TokenStream {
    let method_ident = format_ident!("{}", attrs.method.as_str());
    let path_template = &attrs.path;
    let url_code = generate_blanket_url_code(&attrs.path, params);
    let query_code = generate_query_code(params);
    let headers_code = generate_headers_code(params, user_agent, trait_headers);
    let pre_body_code = generate_pre_body_code(params);
    let body_code = generate_body_code(params);
    let param_metadata_code = generate_parameter_metadata_code(method_name, params);

    // Generate execute code with optional per-method timeout
    let execute_code = if let Some(timeout) = options.timeout {
        let secs = timeout.as_secs();
        let nanos = timeout.subsec_nanos();
        quote! {
            let response = ::tokio::time::timeout(
                ::std::time::Duration::new(#secs, #nanos),
                ::pincer::PincerClient::execute(self, request)
            ).await.map_err(|_| ::pincer::Error::Timeout)??;
        }
    } else {
        quote! {
            let response = ::pincer::PincerClient::execute(self, request).await?;
        }
    };

    // Generate response handling based on return type and options
    let response_handling = generate_response_handling(options, return_type_kind);

    quote! {
        #url_code
        #query_code
        #pre_body_code

        let request = ::pincer::Request::builder(
            ::pincer::Method::#method_ident,
            url,
        )
        #headers_code
        #body_code
        .extension(::pincer::PathTemplate::new(#path_template))
        #param_metadata_code
        .build();

        #execute_code
        #response_handling
    }
}

/// Generate URL building code for blanket impls using `PincerClient::base_url()`.
fn generate_blanket_url_code(path_template: &str, params: &[MethodParam]) -> TokenStream {
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
    let mut path_expr = quote! {
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
        let url = ::pincer::PincerClient::base_url(self).join(&path)
            .map_err(::pincer::Error::InvalidUrl)?;
    }
}

/// Generate the body of a method implementation.
fn generate_method_body(
    attrs: &MethodAttrs,
    params: &[MethodParam],
    user_agent: &str,
    options: &MethodOptions,
    trait_headers: &[(String, String)],
    return_type_kind: ReturnTypeKind,
    method_name: &str,
) -> TokenStream {
    let method_ident = format_ident!("{}", attrs.method.as_str());
    let path_template = &attrs.path;
    let url_code = generate_url_code(&attrs.path, params);
    let query_code = generate_query_code(params);
    let headers_code = generate_headers_code(params, user_agent, trait_headers);
    let pre_body_code = generate_pre_body_code(params);
    let body_code = generate_body_code(params);
    let param_metadata_code = generate_parameter_metadata_code(method_name, params);

    // Generate execute code with optional per-method timeout
    let execute_code = if let Some(timeout) = options.timeout {
        let secs = timeout.as_secs();
        let nanos = timeout.subsec_nanos();
        quote! {
            let response = ::tokio::time::timeout(
                ::std::time::Duration::new(#secs, #nanos),
                self.client.execute(request)
            ).await.map_err(|_| ::pincer::Error::Timeout)??;
        }
    } else {
        quote! {
            let response = self.client.execute(request).await?;
        }
    };

    // Generate response handling based on return type and options
    let response_handling = generate_response_handling(options, return_type_kind);

    quote! {
        #url_code
        #query_code
        #pre_body_code

        let request = ::pincer::Request::builder(
            ::pincer::Method::#method_ident,
            url,
        )
        #headers_code
        #body_code
        .extension(::pincer::PathTemplate::new(#path_template))
        #param_metadata_code
        .build();

        #execute_code
        #response_handling
    }
}

/// Generate response handling code based on return type kind and method options.
fn generate_response_handling(
    options: &MethodOptions,
    return_type_kind: ReturnTypeKind,
) -> TokenStream {
    match (return_type_kind, options.not_found_as_none) {
        // Unit return type: Result<()> - just check for success
        (ReturnTypeKind::Unit, false) => quote! {
            if !response.is_success() {
                return Err(::pincer::Error::http_with_body(
                    response.status(),
                    format!("HTTP error: {}", response.status()),
                    response.into_body(),
                ));
            }
            Ok(())
        },
        // Unit return type with not_found_as_none: Result<Option<()>>
        (ReturnTypeKind::Unit, true) => quote! {
            if response.status() == 404 {
                return Ok(None);
            }
            if !response.is_success() {
                return Err(::pincer::Error::http_with_body(
                    response.status(),
                    format!("HTTP error: {}", response.status()),
                    response.into_body(),
                ));
            }
            Ok(Some(()))
        },
        // Raw response: Result<Response<Bytes>> - return response as-is (no error check)
        (ReturnTypeKind::RawResponse, false) => quote! {
            Ok(response)
        },
        // Raw response with not_found_as_none: Result<Option<Response<Bytes>>>
        (ReturnTypeKind::RawResponse, true) => quote! {
            if response.status() == 404 {
                return Ok(None);
            }
            Ok(Some(response))
        },
        // JSON: Result<T> - deserialize JSON (default behavior)
        (ReturnTypeKind::Json, false) => quote! {
            if !response.is_success() {
                return Err(::pincer::Error::http_with_body(
                    response.status(),
                    format!("HTTP error: {}", response.status()),
                    response.into_body(),
                ));
            }
            response.json()
        },
        // JSON with not_found_as_none: Result<Option<T>>
        (ReturnTypeKind::Json, true) => quote! {
            if response.status() == 404 {
                return Ok(None);
            }
            if !response.is_success() {
                return Err(::pincer::Error::http_with_body(
                    response.status(),
                    format!("HTTP error: {}", response.status()),
                    response.into_body(),
                ));
            }
            response.json().map(Some)
        },
    }
}

// Standalone method attribute macros (used outside of #[pincer] traits)

/// Expand an HTTP method attribute on a standalone method.
pub fn expand_http_method(
    method: HttpMethod,
    attr: TokenStream,
    item: TokenStream,
) -> syn::Result<TokenStream> {
    let path: syn::LitStr = parse2(attr)?;
    let method_fn: syn::ImplItemFn = parse2(item)?;
    let attrs = MethodAttrs::new(method, path.value());
    generate_standalone_method(&method_fn, &attrs)
}

/// Expand a custom `#[http("VERB /path")]` attribute on a standalone method.
pub fn expand_custom_http(attr: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    let spec: syn::LitStr = parse2(attr)?;
    let spec_str = spec.value();

    let (method_str, path_str) = spec_str.split_once(' ').ok_or_else(|| {
        syn::Error::new_spanned(
            &spec,
            "expected format: \"METHOD /path\" (e.g., \"GET /users/{id}\")",
        )
    })?;

    let method = HttpMethod::parse(method_str).ok_or_else(|| {
        syn::Error::new_spanned(
            &spec,
            format!(
                "unsupported HTTP method: {method_str}. Supported: GET, POST, PUT, DELETE, PATCH, HEAD, OPTIONS"
            ),
        )
    })?;

    let method_fn: syn::ImplItemFn = parse2(item)?;
    let attrs = MethodAttrs::new(method, path_str.to_string());
    generate_standalone_method(&method_fn, &attrs)
}

/// Generate a standalone method implementation (for impl blocks outside #[pincer] traits).
fn generate_standalone_method(
    method_fn: &syn::ImplItemFn,
    attrs: &MethodAttrs,
) -> syn::Result<TokenStream> {
    let vis = &method_fn.vis;
    let sig = &method_fn.sig;
    let fn_name = &sig.ident;
    let asyncness = &sig.asyncness;
    let output = &sig.output;

    let params = parse_method_params(&method_fn.sig.inputs, &attrs.path, attrs.method)?;
    let clean_inputs = strip_pincer_attrs(&sig.inputs);

    let path_template = &attrs.path;
    let method_name = fn_name.to_string();
    let url_code = generate_url_code(&attrs.path, &params);
    let query_code = generate_query_code(&params);
    let headers_code = generate_headers_code(&params, DEFAULT_USER_AGENT, &[]);
    let pre_body_code = generate_pre_body_code(&params);
    let body_code = generate_body_code(&params);
    let param_metadata_code = generate_parameter_metadata_code(&method_name, &params);
    let method_ident = format_ident!("{}", attrs.method.as_str());

    Ok(quote! {
        #vis #asyncness fn #fn_name(#clean_inputs) #output {
            #url_code
            #query_code
            #pre_body_code

            let request = ::pincer::Request::builder(
                ::pincer::Method::#method_ident,
                url,
            )
            #headers_code
            #body_code
            .extension(::pincer::PathTemplate::new(#path_template))
            #param_metadata_code
            .build();

            let response = self.client.execute(request).await?;

            if !response.is_success() {
                return Err(::pincer::Error::http(
                    response.status(),
                    format!("HTTP error: {}", response.status()),
                ));
            }

            response.json()
        }
    })
}

/// Parse method parameters from function inputs.
///
/// Uses the same auto-classification logic as trait methods.
fn parse_method_params(
    inputs: &syn::punctuated::Punctuated<FnArg, syn::token::Comma>,
    path_template: &str,
    http_method: HttpMethod,
) -> syn::Result<Vec<MethodParam>> {
    let placeholders = extract_path_placeholders(path_template);
    let mut params = Vec::new();
    let mut unclassified: Vec<(Ident, syn::Type, &syn::PatType)> = Vec::new();

    for arg in inputs {
        if let FnArg::Typed(pat_type) = arg {
            let name = match pat_type.pat.as_ref() {
                Pat::Ident(pat_ident) => pat_ident.ident.clone(),
                _ => continue,
            };
            let ty = (*pat_type.ty).clone();

            // Check for explicit attribute first
            if let Some(kind) = pat_type.attrs.iter().find_map(parse_param_attr) {
                params.push(MethodParam { name, ty, kind });
                continue;
            }

            // Check if param name matches a URL placeholder
            let param_name_str = name.to_string();
            if placeholders.contains(&param_name_str) {
                params.push(MethodParam {
                    name,
                    ty,
                    kind: ParamKind::Path(None),
                });
                continue;
            }

            // No explicit attribute and no placeholder match -> unclassified
            unclassified.push((name, ty, pat_type));
        }
    }

    // Handle unclassified parameters
    match unclassified.len() {
        0 => {}
        1 if http_method.supports_body() => {
            // Single unclassified param becomes Body for body-supporting methods
            if let Some((name, ty, _)) = unclassified.into_iter().next() {
                params.push(MethodParam {
                    name,
                    ty,
                    kind: ParamKind::Body,
                });
            }
        }
        1 => {
            // Non-body method with unclassified param -> error
            if let Some((name, _, pat_type)) = unclassified.first() {
                return Err(syn::Error::new_spanned(
                    pat_type,
                    format!(
                        "parameter '{}' does not match any URL placeholder (available: {:?}) \
                         and {} requests do not support body. \
                         Add #[query] or another explicit attribute.",
                        name,
                        placeholders,
                        http_method.as_str().to_uppercase()
                    ),
                ));
            }
        }
        _ => {
            // Multiple unclassified params -> error
            let names: Vec<_> = unclassified.iter().map(|(n, _, _)| n.to_string()).collect();
            if let Some((_, _, pat_type)) = unclassified.get(1) {
                return Err(syn::Error::new_spanned(
                    pat_type,
                    format!(
                        "multiple unattributed parameters found: {names:?}. \
                         Only one body parameter is allowed. \
                         Add explicit attributes to disambiguate.",
                    ),
                ));
            }
        }
    }

    Ok(params)
}

/// Strip pincer-specific attributes from function parameters.
fn strip_pincer_attrs(
    inputs: &syn::punctuated::Punctuated<FnArg, syn::token::Comma>,
) -> syn::punctuated::Punctuated<FnArg, syn::token::Comma> {
    inputs
        .iter()
        .map(|arg| match arg {
            FnArg::Typed(pat_type) => {
                let mut clean = pat_type.clone();
                clean.attrs.retain(|attr| !is_pincer_param_attr(attr));
                FnArg::Typed(clean)
            }
            FnArg::Receiver(receiver) => FnArg::Receiver(receiver.clone()),
        })
        .collect()
}

/// Generate parameter metadata code for injection into request extensions.
fn generate_parameter_metadata_code(method_name: &str, params: &[MethodParam]) -> TokenStream {
    let param_metas: Vec<_> = params
        .iter()
        .map(|p| {
            let name = p.name.to_string();
            let location = param_kind_to_location(&p.kind);
            let type_name = type_to_string(&p.ty);
            let required = !is_option_type(&p.ty);
            quote! {
                ::pincer::ParamMeta {
                    name: #name,
                    location: #location,
                    type_name: #type_name,
                    required: #required,
                }
            }
        })
        .collect();

    if param_metas.is_empty() {
        quote! {
            .extension(::pincer::ParameterMetadata {
                method_name: #method_name,
                parameters: &[],
            })
        }
    } else {
        quote! {
            .extension(::pincer::ParameterMetadata {
                method_name: #method_name,
                parameters: &[
                    #(#param_metas),*
                ],
            })
        }
    }
}

/// Convert a `ParamKind` to a `ParamLocation` token stream.
fn param_kind_to_location(kind: &ParamKind) -> TokenStream {
    match kind {
        ParamKind::Path(_) => quote! { ::pincer::ParamLocation::Path },
        ParamKind::Query(_) => quote! { ::pincer::ParamLocation::Query },
        ParamKind::Header(_) | ParamKind::Headers => quote! { ::pincer::ParamLocation::Header },
        ParamKind::Body => quote! { ::pincer::ParamLocation::Body },
        ParamKind::Form | ParamKind::Multipart(_) => quote! { ::pincer::ParamLocation::Form },
    }
}

/// Convert a `syn::Type` to a string representation.
fn type_to_string(ty: &syn::Type) -> String {
    quote!(#ty).to_string().replace(' ', "")
}

/// Check if a type is `Option<T>`.
fn is_option_type(ty: &syn::Type) -> bool {
    if let syn::Type::Path(type_path) = ty
        && let Some(segment) = type_path.path.segments.last()
    {
        return segment.ident == "Option";
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    #[test]
    fn parse_pincer_args_basic() {
        let attr: TokenStream = quote! { url = "https://api.example.com" };
        let args = parse_pincer_args(attr).expect("parse");
        assert_eq!(args.url, Some("https://api.example.com".to_string()));
        assert!(args.user_agent.is_none());
        assert_eq!(args.user_agent(), DEFAULT_USER_AGENT);
    }

    #[test]
    fn parse_pincer_args_with_user_agent() {
        let attr: TokenStream =
            quote! { url = "https://api.example.com", user_agent = "my-app/1.0" };
        let args = parse_pincer_args(attr).expect("parse");
        assert_eq!(args.url, Some("https://api.example.com".to_string()));
        assert_eq!(args.user_agent, Some("my-app/1.0".to_string()));
        assert_eq!(args.user_agent(), "my-app/1.0");
    }

    #[test]
    fn parse_pincer_args_missing_url() {
        let attr: TokenStream = quote! { user_agent = "my-app/1.0" };
        let result = parse_pincer_args(attr);
        assert!(result.is_err());
    }

    #[test]
    fn expand_custom_http_get() {
        let attr: TokenStream = quote! { "GET /users/{id}" };
        let item: TokenStream = quote! {
            pub async fn get_user(&self, id: u64) -> pincer::Result<User> { todo!() }
        };
        let result = expand_custom_http(attr, item);
        assert!(
            result.is_ok(),
            "expand_custom_http should succeed for GET: {result:?}"
        );
    }

    #[test]
    fn expand_custom_http_options() {
        let attr: TokenStream = quote! { "OPTIONS /users" };
        let item: TokenStream = quote! {
            pub async fn user_options(&self) -> pincer::Result<()> { todo!() }
        };
        let result = expand_custom_http(attr, item);
        assert!(
            result.is_ok(),
            "expand_custom_http should succeed for OPTIONS"
        );
    }

    #[test]
    fn expand_custom_http_invalid_format() {
        let attr: TokenStream = quote! { "/users" };
        let item: TokenStream = quote! {
            pub async fn get_users(&self) -> pincer::Result<()> { todo!() }
        };
        let result = expand_custom_http(attr, item);
        assert!(
            result.is_err(),
            "expand_custom_http should fail without method"
        );
    }

    #[test]
    fn expand_custom_http_invalid_method() {
        let attr: TokenStream = quote! { "UNKNOWN /users" };
        let item: TokenStream = quote! {
            pub async fn get_users(&self) -> pincer::Result<()> { todo!() }
        };
        let result = expand_custom_http(attr, item);
        assert!(
            result.is_err(),
            "expand_custom_http should fail for unknown method"
        );
    }
}
