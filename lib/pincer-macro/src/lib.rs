//! Procedural macros for pincer declarative HTTP client.
//!
//! This crate provides the proc-macros for declaring HTTP clients:
//! - `#[pincer]` - Mark a trait as a pincer HTTP client
//! - `#[get]`, `#[post]`, `#[put]`, `#[delete]`, `#[patch]`, `#[head]`, `#[options]` - HTTP method attributes
//! - `#[http("VERB /path")]` - Custom HTTP method attribute for extensibility
//! - `#[path]`, `#[query]`, `#[header]`, `#[body]`, `#[form]` - Parameter attributes
//! - `#[derive(Query)]` - Derive macro for struct-based query parameters
//!
//! # Example
//!
//! ```ignore
//! use pincer::prelude::*;
//!
//! #[pincer(url = "https://api.github.com")]
//! pub trait GitHubApi {
//!     #[get("/users/{username}")]
//!     async fn get_user(&self, #[path] username: &str) -> pincer::Result<User>;
//! }
//!
//! // Usage:
//! let client = GitHubApiClient::builder().build();
//! let user = client.get_user("octocat").await?;
//! ```

mod attrs;
mod codegen;
mod expand;
mod query_derive;

use proc_macro::TokenStream;

use crate::attrs::HttpMethod;

/// Mark a trait as a pincer HTTP client.
///
/// This macro generates:
/// - A clean trait (without pincer attributes)
/// - A client struct implementing the trait (e.g., `GitHubApiClient`)
/// - A builder struct for constructing the client (e.g., `GitHubApiClientBuilder`)
///
/// # Attributes
///
/// - `url` (required): The base URL for the client
/// - `user_agent` (optional): Custom User-Agent header
///
/// # Example
///
/// ```ignore
/// #[pincer(url = "https://api.github.com")]
/// pub trait GitHubApi {
///     #[get("/users/{username}")]
///     async fn get_user(&self, #[path] username: &str) -> pincer::Result<User>;
/// }
///
/// // Usage:
/// let client = GitHubApiClientBuilder::default().build();
/// let user = client.get_user("octocat").await?;
/// ```
#[proc_macro_attribute]
pub fn pincer(attr: TokenStream, item: TokenStream) -> TokenStream {
    expand::expand_pincer_trait(attr.into(), item.into())
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

/// Mark a method as a GET request.
///
/// # Example
///
/// ```ignore
/// #[get("/users/{id}")]
/// pub async fn get_user(&self, #[path] id: u64) -> pincer::Result<User>;
/// ```
#[proc_macro_attribute]
pub fn get(attr: TokenStream, item: TokenStream) -> TokenStream {
    expand::expand_http_method(HttpMethod::Get, attr.into(), item.into())
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

/// Mark a method as a POST request.
///
/// # Example
///
/// ```ignore
/// #[post("/users")]
/// pub async fn create_user(&self, #[body] user: &CreateUser) -> pincer::Result<User>;
/// ```
#[proc_macro_attribute]
pub fn post(attr: TokenStream, item: TokenStream) -> TokenStream {
    expand::expand_http_method(HttpMethod::Post, attr.into(), item.into())
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

/// Mark a method as a PUT request.
///
/// # Example
///
/// ```ignore
/// #[put("/users/{id}")]
/// pub async fn update_user(&self, #[path] id: u64, #[body] user: &UpdateUser) -> pincer::Result<User>;
/// ```
#[proc_macro_attribute]
pub fn put(attr: TokenStream, item: TokenStream) -> TokenStream {
    expand::expand_http_method(HttpMethod::Put, attr.into(), item.into())
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

/// Mark a method as a DELETE request.
///
/// # Example
///
/// ```ignore
/// #[delete("/users/{id}")]
/// pub async fn delete_user(&self, #[path] id: u64) -> pincer::Result<()>;
/// ```
#[proc_macro_attribute]
pub fn delete(attr: TokenStream, item: TokenStream) -> TokenStream {
    expand::expand_http_method(HttpMethod::Delete, attr.into(), item.into())
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

/// Mark a method as a PATCH request.
///
/// # Example
///
/// ```ignore
/// #[patch("/users/{id}")]
/// pub async fn patch_user(&self, #[path] id: u64, #[body] patch: &PatchUser) -> pincer::Result<User>;
/// ```
#[proc_macro_attribute]
pub fn patch(attr: TokenStream, item: TokenStream) -> TokenStream {
    expand::expand_http_method(HttpMethod::Patch, attr.into(), item.into())
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

/// Mark a method as a HEAD request.
///
/// # Example
///
/// ```ignore
/// #[head("/users/{id}")]
/// pub async fn check_user(&self, #[path] id: u64) -> pincer::Result<()>;
/// ```
#[proc_macro_attribute]
pub fn head(attr: TokenStream, item: TokenStream) -> TokenStream {
    expand::expand_http_method(HttpMethod::Head, attr.into(), item.into())
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

/// Mark a method as an OPTIONS request.
///
/// # Example
///
/// ```ignore
/// #[options("/users")]
/// pub async fn user_options(&self) -> pincer::Result<()>;
/// ```
#[proc_macro_attribute]
pub fn options(attr: TokenStream, item: TokenStream) -> TokenStream {
    expand::expand_http_method(HttpMethod::Options, attr.into(), item.into())
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

/// Mark a method with a custom HTTP method and path.
///
/// This is useful for less common HTTP methods or when you want to
/// specify the method and path in a single attribute.
///
/// # Example
///
/// ```ignore
/// #[http("GET /users/{id}")]
/// pub async fn get_user(&self, #[path] id: u64) -> pincer::Result<User>;
///
/// #[http("OPTIONS /users")]
/// pub async fn user_options(&self) -> pincer::Result<()>;
/// ```
#[proc_macro_attribute]
pub fn http(attr: TokenStream, item: TokenStream) -> TokenStream {
    expand::expand_custom_http(attr.into(), item.into())
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

/// Derive the `ToQueryPairs` trait for a struct.
///
/// This generates a method to convert the struct into query parameter pairs.
///
/// # Struct Attributes
///
/// - `#[query(rename_all = "camelCase")]` - Rename all fields using a case convention
///
/// Supported case conventions:
/// - `lowercase`, `UPPERCASE`
/// - `camelCase`, `PascalCase`
/// - `snake_case`, `SCREAMING_SNAKE_CASE`
/// - `kebab-case`, `SCREAMING-KEBAB-CASE`
///
/// # Field Attributes
///
/// - `#[query(skip_none)]` - Skip the field if it's `None` (default for `Option<T>`)
/// - `#[query(rename = "name")]` - Use a different name in the query string (overrides `rename_all`)
/// - `#[query(format = "csv")]` - Collection format for `Vec<T>` (csv, ssv, pipes, multi)
///
/// # Example
///
/// ```ignore
/// use pincer::Query;
///
/// #[derive(Query)]
/// #[query(rename_all = "camelCase")]
/// struct SearchParams {
///     search_query: String,      // becomes "searchQuery"
///     page_number: Option<u32>,  // becomes "pageNumber"
///     #[query(rename = "limit")] // explicit rename overrides rename_all
///     per_page: u32,
///     #[query(format = "csv")]
///     tag_list: Vec<String>,     // becomes "tagList"
/// }
/// ```
#[proc_macro_derive(Query, attributes(query))]
pub fn derive_query(input: TokenStream) -> TokenStream {
    query_derive::expand_query_derive(input.into())
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}
