//! Attribute parsing for pincer proc-macros.

use syn::{Ident, Type};

/// HTTP method for a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
    Patch,
    Head,
    Options,
}

impl HttpMethod {
    /// Get the method name as a token for code generation.
    #[must_use]
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Get => "Get",
            Self::Post => "Post",
            Self::Put => "Put",
            Self::Delete => "Delete",
            Self::Patch => "Patch",
            Self::Head => "Head",
            Self::Options => "Options",
        }
    }

    /// Parse an HTTP method from a string (case-insensitive).
    /// Returns `None` for unsupported methods.
    #[must_use]
    pub(crate) fn parse(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "GET" => Some(Self::Get),
            "POST" => Some(Self::Post),
            "PUT" => Some(Self::Put),
            "DELETE" => Some(Self::Delete),
            "PATCH" => Some(Self::Patch),
            "HEAD" => Some(Self::Head),
            "OPTIONS" => Some(Self::Options),
            _ => None,
        }
    }

    /// Returns true if this HTTP method typically has a request body.
    #[must_use]
    pub(crate) const fn supports_body(self) -> bool {
        matches!(self, Self::Post | Self::Put | Self::Patch)
    }
}

/// Generation mode for the `#[pincer]` macro.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum PincerMode {
    /// Full mode (default): generates a concrete client struct with `HyperClient`
    /// and a builder struct.
    #[default]
    Full,

    /// Wrapper mode: generates a generic client struct `Client<C>` that works
    /// with any `PincerClient`. No builder is generated.
    Wrapper,

    /// Impl-only mode: only generates the trait implementation for any type
    /// implementing `PincerClient`. No structs are generated.
    ImplOnly,
}

impl PincerMode {
    /// Parse a mode from a string.
    #[must_use]
    pub(crate) fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "full" => Some(Self::Full),
            "wrapper" => Some(Self::Wrapper),
            "impl_only" | "implonly" | "impl-only" => Some(Self::ImplOnly),
            _ => None,
        }
    }
}

/// Attributes for HTTP method macros (`#[get]`, `#[post]`, etc.).
#[derive(Debug)]
pub(crate) struct MethodAttrs {
    /// The HTTP method.
    pub(crate) method: HttpMethod,
    /// The path template (e.g., "/users/{id}").
    pub(crate) path: String,
}

impl MethodAttrs {
    /// Parse method attributes from the path string.
    pub(crate) fn new(method: HttpMethod, path: String) -> Self {
        Self { method, path }
    }
}

/// Collection format for serializing `Vec<T>` query parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum CollectionFormat {
    /// Repeated parameters: `?tags=a&tags=b&tags=c` (default)
    #[default]
    Multi,
    /// Comma-separated: `?tags=a,b,c`
    Csv,
    /// Space-separated: `?tags=a%20b%20c`
    Ssv,
    /// Pipe-separated: `?tags=a|b|c`
    Pipes,
}

impl CollectionFormat {
    /// Get the separator string for this format.
    ///
    /// Returns `None` for `Multi` format (uses repeated parameters).
    #[must_use]
    pub(crate) const fn separator(self) -> Option<&'static str> {
        match self {
            Self::Multi => None,
            Self::Csv => Some(","),
            Self::Ssv => Some(" "),
            Self::Pipes => Some("|"),
        }
    }
}

/// Query parameter options.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct QueryOptions {
    /// Optional alias for the query parameter name.
    pub(crate) alias: Option<String>,
    /// Collection format for Vec<T> parameters.
    pub(crate) format: CollectionFormat,
}

/// Multipart parameter options.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct MultipartOptions {
    /// Optional field name override.
    pub(crate) name: Option<String>,
}

/// Parameter kind for method arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ParamKind {
    /// Path parameter (e.g., `#[path]` or `#[path("user_id")]`).
    Path(Option<String>),
    /// Query parameter (e.g., `#[query]`, `#[query("page_size")]`, `#[query(format = "csv")]`).
    Query(QueryOptions),
    /// Header parameter (e.g., `#[header("Authorization")]`).
    Header(String),
    /// Header map for dynamic headers (e.g., `#[headers]`).
    /// Expects a type implementing `IntoIterator<Item = (K, V)>` where K and V are string-like.
    Headers,
    /// JSON body (e.g., `#[body]`).
    Body,
    /// Form body (e.g., `#[form]`).
    Form,
    /// Multipart form part (e.g., `#[multipart]` or `#[multipart(name = "file")]`).
    /// Expects a `Part` or `Vec<Part>` type.
    Multipart(MultipartOptions),
}

/// A parsed method parameter.
#[derive(Debug)]
pub(crate) struct MethodParam {
    /// Parameter name from the function signature.
    pub(crate) name: Ident,
    /// Parameter type.
    pub(crate) ty: Type,
    /// Parameter kind (path, query, header, body).
    pub(crate) kind: ParamKind,
}

/// Method-level options parsed from attributes.
///
/// These options modify how the generated method behaves.
#[derive(Debug, Clone, Default)]
pub(crate) struct MethodOptions {
    /// Treat 404 responses as `None` instead of an error.
    ///
    /// When enabled, the method should return `Result<Option<T>>` instead of `Result<T>`,
    /// and a 404 status code will return `Ok(None)` rather than an error.
    pub(crate) not_found_as_none: bool,

    /// Per-method timeout override.
    ///
    /// When set, overrides the client's default timeout for this specific method.
    pub(crate) timeout: Option<std::time::Duration>,
}

/// Parse method-level options from attributes.
///
/// Recognized attributes:
/// - `#[not_found_as_none]` - Treat 404 as None
/// - `#[timeout("30s")]` or `#[timeout(secs = 30)]` - Per-method timeout
pub(crate) fn parse_method_options(attrs: &[syn::Attribute]) -> syn::Result<MethodOptions> {
    let mut options = MethodOptions::default();

    for attr in attrs {
        let path = attr.path();

        if path.is_ident("not_found_as_none") {
            options.not_found_as_none = true;
        }

        if path.is_ident("timeout")
            && let Some(duration) = parse_duration_attr(attr)?
        {
            options.timeout = Some(duration);
        }
    }

    Ok(options)
}

/// Parse a duration from an attribute like `#[timeout("30s")]` or `#[timeout(secs = 30)]`.
fn parse_duration_attr(attr: &syn::Attribute) -> syn::Result<Option<std::time::Duration>> {
    match &attr.meta {
        syn::Meta::List(meta_list) => {
            // Try parsing as string like "30s", "1m", "500ms"
            if let Ok(str_lit) = syn::parse2::<syn::LitStr>(meta_list.tokens.clone()) {
                let value = str_lit.value();
                return parse_duration_string(&value).map(Some).ok_or_else(|| {
                    syn::Error::new_spanned(
                        &str_lit,
                        "invalid duration format. Expected: \"30s\", \"1m\", \"500ms\"",
                    )
                });
            }

            // Try parsing as `secs = N` or `millis = N`
            if let Ok(name_value) = syn::parse2::<syn::MetaNameValue>(meta_list.tokens.clone()) {
                if name_value.path.is_ident("secs")
                    && let syn::Expr::Lit(syn::ExprLit {
                        lit: syn::Lit::Int(lit_int),
                        ..
                    }) = &name_value.value
                {
                    let secs: u64 = lit_int.base10_parse()?;
                    return Ok(Some(std::time::Duration::from_secs(secs)));
                }
                if name_value.path.is_ident("millis")
                    && let syn::Expr::Lit(syn::ExprLit {
                        lit: syn::Lit::Int(lit_int),
                        ..
                    }) = &name_value.value
                {
                    let millis: u64 = lit_int.base10_parse()?;
                    return Ok(Some(std::time::Duration::from_millis(millis)));
                }
            }

            Err(syn::Error::new_spanned(
                &meta_list.tokens,
                "expected duration string like \"30s\" or `secs = 30`",
            ))
        }
        _ => Ok(None),
    }
}

/// Parse a duration string like "30s", "1m", "500ms".
fn parse_duration_string(s: &str) -> Option<std::time::Duration> {
    let s = s.trim();

    if let Some(secs) = s.strip_suffix('s') {
        if let Some(millis) = secs.strip_suffix('m') {
            // "500ms"
            let ms: u64 = millis.parse().ok()?;
            return Some(std::time::Duration::from_millis(ms));
        }
        // "30s"
        let secs: u64 = secs.parse().ok()?;
        return Some(std::time::Duration::from_secs(secs));
    }

    if let Some(mins) = s.strip_suffix('m') {
        // "1m"
        let mins: u64 = mins.parse().ok()?;
        return Some(std::time::Duration::from_secs(mins * 60));
    }

    None
}

/// Parse a parameter attribute and return its kind.
pub(crate) fn parse_param_attr(attr: &syn::Attribute) -> Option<ParamKind> {
    let path = attr.path();

    if path.is_ident("path") {
        let name = parse_optional_string_arg(attr);
        return Some(ParamKind::Path(name));
    }

    if path.is_ident("query") {
        let options = parse_query_options(attr);
        return Some(ParamKind::Query(options));
    }

    if path.is_ident("header") {
        let name = parse_required_string_arg(attr)?;
        return Some(ParamKind::Header(name));
    }

    if path.is_ident("body") {
        return Some(ParamKind::Body);
    }

    if path.is_ident("form") {
        return Some(ParamKind::Form);
    }

    if path.is_ident("headers") {
        return Some(ParamKind::Headers);
    }

    if path.is_ident("multipart") {
        let options = parse_multipart_options(attr);
        return Some(ParamKind::Multipart(options));
    }

    None
}

/// Parse multipart parameter options from `#[multipart]` or `#[multipart(name = "file")]`.
fn parse_multipart_options(attr: &syn::Attribute) -> MultipartOptions {
    let mut options = MultipartOptions::default();

    if let syn::Meta::List(meta_list) = &attr.meta {
        // Try parsing as key-value pairs (name = "file")
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("name") {
                let value: syn::LitStr = meta.value()?.parse()?;
                options.name = Some(value.value());
            }
            Ok(())
        });

        // Also try parsing as a simple string literal for convenience
        if options.name.is_none()
            && let Ok(str_lit) = syn::parse2::<syn::LitStr>(meta_list.tokens.clone())
        {
            options.name = Some(str_lit.value());
        }
    }

    options
}

/// Parse query parameter options from `#[query]`, `#[query("alias")]`, or `#[query(format = "csv")]`.
fn parse_query_options(attr: &syn::Attribute) -> QueryOptions {
    let mut options = QueryOptions::default();

    if let syn::Meta::List(meta_list) = &attr.meta {
        // Try parsing as a simple string literal first (alias)
        if let Ok(str_lit) = syn::parse2::<syn::LitStr>(meta_list.tokens.clone()) {
            options.alias = Some(str_lit.value());
            return options;
        }

        // Try parsing as key-value pairs (format = "csv", etc.)
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("format") {
                let value: syn::LitStr = meta.value()?.parse()?;
                options.format = parse_collection_format(&value.value());
            } else if let Some(ident) = meta.path.get_ident() {
                // Could be an alias specified as an identifier
                options.alias = Some(ident.to_string());
            }
            Ok(())
        });
    }
    // Other cases (Path, NameValue) - no options to parse

    options
}

/// Parse a collection format string.
fn parse_collection_format(s: &str) -> CollectionFormat {
    match s.to_lowercase().as_str() {
        "csv" | "comma" => CollectionFormat::Csv,
        "ssv" | "space" => CollectionFormat::Ssv,
        "pipes" | "pipe" => CollectionFormat::Pipes,
        _ => CollectionFormat::Multi, // default
    }
}

/// Parse an optional string argument from an attribute.
fn parse_optional_string_arg(attr: &syn::Attribute) -> Option<String> {
    let meta = attr.meta.clone();
    match meta {
        syn::Meta::List(meta_list) => {
            let tokens = meta_list.tokens;
            let str_lit: syn::LitStr = syn::parse2(tokens).ok()?;
            Some(str_lit.value())
        }
        _ => None,
    }
}

/// Parse a required string argument from an attribute.
fn parse_required_string_arg(attr: &syn::Attribute) -> Option<String> {
    parse_optional_string_arg(attr)
}

/// Extract placeholder names from a URL path template.
///
/// E.g., `/users/{id}/posts/{post_id}` returns `["id", "post_id"]`
#[must_use]
pub(crate) fn extract_path_placeholders(path: &str) -> Vec<String> {
    let mut placeholders = Vec::new();
    let mut chars = path.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '{' {
            let mut name = String::new();
            for next in chars.by_ref() {
                if next == '}' {
                    break;
                }
                name.push(next);
            }
            if !name.is_empty() {
                placeholders.push(name);
            }
        }
    }
    placeholders
}

/// Parse trait-level headers from a `#[headers(...)]` attribute.
///
/// Syntax: `#[headers(X_Api_Version = "v1", Accept = "application/json")]`
///
/// Note: Underscores in header names are converted to hyphens, so `X_Api_Version`
/// becomes `X-Api-Version`.
///
/// Returns a list of (`header_name`, `header_value`) pairs.
pub(crate) fn parse_trait_headers(attrs: &[syn::Attribute]) -> syn::Result<Vec<(String, String)>> {
    let mut headers = Vec::new();

    for attr in attrs {
        if attr.path().is_ident("headers") {
            // Parse the attribute tokens as a list of "key" = "value" pairs
            attr.parse_nested_meta(|meta| {
                // The key is the path - underscores are converted to hyphens for HTTP header names
                let key = if let Some(ident) = meta.path.get_ident() {
                    ident.to_string().replace('_', "-")
                } else {
                    // For path segments (shouldn't happen but handle it)
                    meta.path
                        .segments
                        .iter()
                        .map(|s| s.ident.to_string())
                        .collect::<Vec<_>>()
                        .join("-")
                };

                // Expect "=" followed by a string value
                let value: syn::LitStr = meta.value()?.parse()?;

                headers.push((key, value.value()));
                Ok(())
            })?;
        }
    }

    Ok(headers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_method_as_str() {
        assert_eq!(HttpMethod::Get.as_str(), "Get");
        assert_eq!(HttpMethod::Post.as_str(), "Post");
        assert_eq!(HttpMethod::Delete.as_str(), "Delete");
    }

    #[test]
    fn parse_duration_seconds() {
        assert_eq!(
            parse_duration_string("30s"),
            Some(std::time::Duration::from_secs(30))
        );
        assert_eq!(
            parse_duration_string("1s"),
            Some(std::time::Duration::from_secs(1))
        );
        assert_eq!(
            parse_duration_string("0s"),
            Some(std::time::Duration::from_secs(0))
        );
    }

    #[test]
    fn parse_duration_minutes() {
        assert_eq!(
            parse_duration_string("1m"),
            Some(std::time::Duration::from_secs(60))
        );
        assert_eq!(
            parse_duration_string("5m"),
            Some(std::time::Duration::from_secs(300))
        );
    }

    #[test]
    fn parse_duration_milliseconds() {
        assert_eq!(
            parse_duration_string("500ms"),
            Some(std::time::Duration::from_millis(500))
        );
        assert_eq!(
            parse_duration_string("100ms"),
            Some(std::time::Duration::from_millis(100))
        );
    }

    #[test]
    fn parse_duration_invalid() {
        assert_eq!(parse_duration_string("30"), None);
        assert_eq!(parse_duration_string("abc"), None);
        assert_eq!(parse_duration_string(""), None);
    }

    #[test]
    fn extract_placeholders_single() {
        assert_eq!(
            extract_path_placeholders("/users/{id}"),
            vec!["id".to_string()]
        );
    }

    #[test]
    fn extract_placeholders_multiple() {
        assert_eq!(
            extract_path_placeholders("/repos/{owner}/{repo}/issues/{number}"),
            vec![
                "owner".to_string(),
                "repo".to_string(),
                "number".to_string()
            ]
        );
    }

    #[test]
    fn extract_placeholders_none() {
        assert!(extract_path_placeholders("/health").is_empty());
        assert!(extract_path_placeholders("/users").is_empty());
    }

    #[test]
    fn http_method_supports_body() {
        assert!(HttpMethod::Post.supports_body());
        assert!(HttpMethod::Put.supports_body());
        assert!(HttpMethod::Patch.supports_body());
        assert!(!HttpMethod::Get.supports_body());
        assert!(!HttpMethod::Delete.supports_body());
        assert!(!HttpMethod::Head.supports_body());
        assert!(!HttpMethod::Options.supports_body());
    }
}
