//! Core types and traits for pincer declarative HTTP client.
//!
//! This crate provides the foundational types used by pincer:
//! - [`Method`] - HTTP method enum
//! - [`Request`] and [`RequestBuilder`] - HTTP request types
//! - [`Response`] - HTTP response type
//! - [`Error`] and [`Result`] - Error handling
//! - [`HttpClient`] - Core client trait for HTTP execution
//! - [`PincerClient`] - Extended client trait with base URL support
//! - [`StatusCode`] - HTTP status codes (re-exported from `http` crate)
//! - [`header`] - HTTP header names (re-exported from `http` crate)
//! - [`ToQueryPairs`] - Trait for converting types to query parameter pairs
//! - [`PathTemplate`] - Original path template for middleware access

mod body;
mod client;
mod error;
mod method;
mod multipart;
mod param_meta;
mod path_template;
pub mod prelude;
mod request;
mod response;

pub use body::{ContentType, from_json, to_form, to_json, to_query_string};
pub use client::{HttpClient, HttpClientExt, PincerClient};
pub use error::{DefaultErrorDecoder, Error, ErrorDecoder, Result};
pub use method::Method;
pub use multipart::{Form, Part};
pub use param_meta::{ParamLocation, ParamMeta, ParameterMetadata};
pub use path_template::PathTemplate;
pub use request::{Request, RequestBuilder};
pub use response::Response;

// Re-export http crate types for status codes and headers
pub use http::{StatusCode, header};

#[cfg(feature = "streaming")]
pub use client::HttpClientStreaming;
#[cfg(feature = "streaming")]
pub use response::streaming::{StreamingBody, StreamingResponse};

/// Trait for types that can be converted to query parameter pairs.
///
/// This is automatically implemented by the `#[derive(Query)]` macro.
///
/// # Example
///
/// ```ignore
/// use pincer::Query;
///
/// #[derive(Query)]
/// struct SearchParams {
///     q: String,
///     #[query(skip_none)]
///     page: Option<u32>,
///     #[query(rename = "page_size")]
///     limit: u32,
///     #[query(format = "csv")]
///     tags: Vec<String>,
/// }
/// ```
pub trait ToQueryPairs {
    /// Convert this type to a vector of key-value pairs for query parameters.
    fn to_query_pairs(&self) -> Vec<(String, String)>;
}
