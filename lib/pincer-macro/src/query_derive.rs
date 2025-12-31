//! Query derive macro implementation.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{DeriveInput, Fields, Type, parse2};

/// Struct-level options parsed from `#[query(...)]` attributes.
#[derive(Debug, Clone, Default)]
struct QueryStructOptions {
    /// Rename all fields using the given case convention.
    rename_all: Option<RenameRule>,
}

/// Case conversion rules for `rename_all`.
#[derive(Debug, Clone, Copy)]
#[allow(clippy::enum_variant_names)]
enum RenameRule {
    /// `lowercase`
    LowerCase,
    /// `UPPERCASE`
    UpperCase,
    /// `camelCase`
    CamelCase,
    /// `PascalCase`
    PascalCase,
    /// `snake_case`
    SnakeCase,
    /// `SCREAMING_SNAKE_CASE`
    ScreamingSnakeCase,
    /// `kebab-case`
    KebabCase,
    /// `SCREAMING-KEBAB-CASE`
    ScreamingKebabCase,
}

impl RenameRule {
    /// Parse a rename rule from a string.
    fn parse(s: &str) -> Option<Self> {
        match s {
            "lowercase" => Some(Self::LowerCase),
            "UPPERCASE" => Some(Self::UpperCase),
            "camelCase" => Some(Self::CamelCase),
            "PascalCase" => Some(Self::PascalCase),
            "snake_case" => Some(Self::SnakeCase),
            "SCREAMING_SNAKE_CASE" => Some(Self::ScreamingSnakeCase),
            "kebab-case" => Some(Self::KebabCase),
            "SCREAMING-KEBAB-CASE" => Some(Self::ScreamingKebabCase),
            _ => None,
        }
    }

    /// Apply the rename rule to a field name.
    fn apply(self, name: &str) -> String {
        match self {
            Self::LowerCase => name.to_lowercase(),
            Self::UpperCase => name.to_uppercase(),
            Self::CamelCase => to_camel_case(name),
            Self::PascalCase => to_pascal_case(name),
            Self::SnakeCase => to_snake_case(name),
            Self::ScreamingSnakeCase => to_snake_case(name).to_uppercase(),
            Self::KebabCase => to_snake_case(name).replace('_', "-"),
            Self::ScreamingKebabCase => to_snake_case(name).to_uppercase().replace('_', "-"),
        }
    }
}

/// Convert a string to `snake_case`.
fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                result.push('_');
            }
            result.push(c.to_lowercase().next().unwrap_or(c));
        } else {
            result.push(c);
        }
    }
    result
}

/// Convert a string to `camelCase`.
fn to_camel_case(s: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = false;
    for c in s.chars() {
        if c == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            result.push(c.to_uppercase().next().unwrap_or(c));
            capitalize_next = false;
        } else {
            result.push(c);
        }
    }
    result
}

/// Convert a string to `PascalCase`.
fn to_pascal_case(s: &str) -> String {
    let camel = to_camel_case(s);
    let mut chars = camel.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// Field options parsed from `#[query(...)]` attributes.
#[derive(Debug, Clone, Default)]
struct QueryFieldOptions {
    /// Skip this field if it's None
    skip_none: bool,
    /// Rename the field in query string
    rename: Option<String>,
    /// Collection format for Vec<T> fields
    format: Option<String>,
}

/// Expand the `#[derive(Query)]` macro.
pub fn expand_query_derive(input: TokenStream) -> syn::Result<TokenStream> {
    let input: DeriveInput = parse2(input)?;
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    // Parse struct-level options
    let struct_options = parse_query_struct_options(&input.attrs)?;

    // Only support structs with named fields
    let fields = match &input.data {
        syn::Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => {
                return Err(syn::Error::new_spanned(
                    &input,
                    "Query derive only supports structs with named fields",
                ));
            }
        },
        _ => {
            return Err(syn::Error::new_spanned(
                &input,
                "Query derive only supports structs",
            ));
        }
    };

    let mut field_handlers = Vec::new();

    for field in fields {
        // Safe: we've already verified this is a struct with named fields
        let Some(field_name) = field.ident.as_ref() else {
            continue;
        };
        let field_ty = &field.ty;
        let options = parse_query_field_options(&field.attrs)?;

        // Determine the key: explicit rename > rename_all > field name
        let key = if let Some(ref rename) = options.rename {
            rename.clone()
        } else if let Some(rule) = struct_options.rename_all {
            rule.apply(&field_name.to_string())
        } else {
            field_name.to_string()
        };

        let handler = generate_field_handler(field_name, field_ty, &key, &options);
        field_handlers.push(handler);
    }

    Ok(quote! {
        impl #impl_generics ::pincer::ToQueryPairs for #name #ty_generics #where_clause {
            fn to_query_pairs(&self) -> ::std::vec::Vec<(::std::string::String, ::std::string::String)> {
                let mut pairs = ::std::vec::Vec::new();
                #(#field_handlers)*
                pairs
            }
        }
    })
}

/// Parse struct-level options from `#[query(...)]` attributes.
fn parse_query_struct_options(attrs: &[syn::Attribute]) -> syn::Result<QueryStructOptions> {
    let mut options = QueryStructOptions::default();

    for attr in attrs {
        if !attr.path().is_ident("query") {
            continue;
        }

        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("rename_all") {
                let value: syn::LitStr = meta.value()?.parse()?;
                let rule = RenameRule::parse(&value.value()).ok_or_else(|| {
                    syn::Error::new_spanned(
                        &value,
                        format!(
                            "unknown rename_all value: \"{}\". Expected one of: \
                             lowercase, UPPERCASE, camelCase, PascalCase, \
                             snake_case, SCREAMING_SNAKE_CASE, kebab-case, SCREAMING-KEBAB-CASE",
                            value.value()
                        ),
                    )
                })?;
                options.rename_all = Some(rule);
            }
            Ok(())
        })?;
    }

    Ok(options)
}

/// Parse field options from `#[query(...)]` attributes.
fn parse_query_field_options(attrs: &[syn::Attribute]) -> syn::Result<QueryFieldOptions> {
    let mut options = QueryFieldOptions::default();

    for attr in attrs {
        if !attr.path().is_ident("query") {
            continue;
        }

        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("skip_none") {
                options.skip_none = true;
            } else if meta.path.is_ident("rename") {
                let value: syn::LitStr = meta.value()?.parse()?;
                options.rename = Some(value.value());
            } else if meta.path.is_ident("format") {
                let value: syn::LitStr = meta.value()?.parse()?;
                options.format = Some(value.value());
            }
            Ok(())
        })?;
    }

    Ok(options)
}

/// Generate code for handling a single field.
fn generate_field_handler(
    field_name: &syn::Ident,
    field_ty: &Type,
    key: &str,
    options: &QueryFieldOptions,
) -> TokenStream {
    let is_option = is_option_type(field_ty);
    let is_vec = is_vec_type(field_ty);

    if is_option {
        // Option<T>: skip if None (skip_none is default behavior for Option)
        quote! {
            if let Some(ref value) = self.#field_name {
                pairs.push((#key.to_string(), value.to_string()));
            }
        }
    } else if is_vec {
        let format = options.format.as_deref().unwrap_or("multi");
        match format {
            "csv" | "comma" => quote! {
                if !self.#field_name.is_empty() {
                    let value = self.#field_name.iter()
                        .map(|x| x.to_string())
                        .collect::<::std::vec::Vec<_>>()
                        .join(",");
                    pairs.push((#key.to_string(), value));
                }
            },
            "ssv" | "space" => quote! {
                if !self.#field_name.is_empty() {
                    let value = self.#field_name.iter()
                        .map(|x| x.to_string())
                        .collect::<::std::vec::Vec<_>>()
                        .join(" ");
                    pairs.push((#key.to_string(), value));
                }
            },
            "pipes" | "pipe" => quote! {
                if !self.#field_name.is_empty() {
                    let value = self.#field_name.iter()
                        .map(|x| x.to_string())
                        .collect::<::std::vec::Vec<_>>()
                        .join("|");
                    pairs.push((#key.to_string(), value));
                }
            },
            _ => {
                // "multi" - repeated parameters
                quote! {
                    for item in &self.#field_name {
                        pairs.push((#key.to_string(), item.to_string()));
                    }
                }
            }
        }
    } else {
        // Simple type
        quote! {
            pairs.push((#key.to_string(), self.#field_name.to_string()));
        }
    }
}

/// Check if a type is `Option<T>`.
fn is_option_type(ty: &Type) -> bool {
    matches!(ty, Type::Path(type_path)
        if type_path.path.segments.last()
            .is_some_and(|seg| seg.ident == "Option"))
}

/// Check if a type is `Vec<T>`.
fn is_vec_type(ty: &Type) -> bool {
    matches!(ty, Type::Path(type_path)
        if type_path.path.segments.last()
            .is_some_and(|seg| seg.ident == "Vec"))
}
