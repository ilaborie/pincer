//! Declarative HTTP client for Rust.
//!
//! Define HTTP clients using proc-macros with async/await and Tower middleware.
//!
//! # Example
//!
//! ```ignore
//! use pincer::prelude::*;
//!
//! #[derive(Debug, Deserialize)]
//! pub struct User {
//!     id: u64,
//!     name: String,
//! }
//!
//! #[pincer(url = "https://api.example.com")]
//! pub trait UserApi {
//!     #[get("/users/{id}")]
//!     async fn get_user(&self, #[path] id: u64) -> pincer::Result<User>;
//! }
//!
//! let client = UserApiClientBuilder::default().build()?;
//! let user = client.get_user(42).await?;
//! ```
//!
//! See the [tutorial][_tutorial] for a complete guide.

pub mod _tutorial;
mod api_client;
mod client;
mod config;
mod connector;
pub mod middleware;
pub mod prelude;

// Re-export client types
pub use api_client::ApiClient;
pub use client::{HyperClient, HyperClientBuilder, ServiceFuture};
pub use config::{ClientConfig, ClientConfigBuilder};

// Re-export tower for middleware composition
pub use tower;

// Re-export core types
pub use pincer_core::{
    ContentType, DefaultErrorDecoder, Error, ErrorDecoder, Form, HttpClient, HttpClientExt, Method,
    Part, PincerClient, Request, RequestBuilder, Response, Result, ToQueryPairs, from_json,
    to_form, to_json, to_query_string,
};

// Re-export http types for status codes and headers
pub use pincer_core::{StatusCode, header};

// Note: Form and Part are re-exported from pincer_core at the crate root

// Re-export streaming types (feature-gated)
#[cfg(feature = "streaming")]
pub use pincer_core::{HttpClientStreaming, StreamingBody, StreamingResponse};

// Re-export crates for macro-generated code
pub use percent_encoding;
pub use serde_html_form;
pub use url;

// Re-export macros
pub use pincer_macro::{Query, delete, get, head, http, options, patch, pincer, post, put};
