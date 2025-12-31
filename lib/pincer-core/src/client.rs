//! HTTP client traits.
//!
//! - [`HttpClient`] - Low-level HTTP execution
//! - [`PincerClient`] - High-level client with base URL (for `#[pincer]` macro)
//!
//! Most users should use the `#[pincer]` macro which generates clients automatically.
//! Implement [`PincerClient`] directly for custom auth or testing.

use std::future::Future;

use bytes::Bytes;
use url::Url;

use crate::{Request, Response, Result};

/// Core HTTP client trait.
///
/// This trait defines the interface for executing HTTP requests.
/// Implementations should be async-first and support connection pooling.
pub trait HttpClient: Send + Sync {
    /// Execute an HTTP request and return the response.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails for any reason:
    /// - Network errors
    /// - TLS errors
    /// - Timeouts
    /// - Invalid response
    fn execute(
        &self,
        request: Request<Bytes>,
    ) -> impl Future<Output = Result<Response<Bytes>>> + Send;
}

/// Extension trait for [`HttpClient`] with convenience methods.
pub trait HttpClientExt: HttpClient {
    /// Execute a GET request.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails.
    fn get(&self, url: &str) -> impl Future<Output = Result<Response<Bytes>>> + Send {
        async move {
            let url = url::Url::parse(url)?;
            let request = Request::builder(crate::Method::Get, url).build();
            self.execute(request).await
        }
    }

    /// Execute a POST request with a JSON body.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization or the request fails.
    fn post_json<T: serde::Serialize + Send + Sync>(
        &self,
        url: &str,
        body: &T,
    ) -> impl Future<Output = Result<Response<Bytes>>> + Send {
        async move {
            let url = url::Url::parse(url)?;
            let request = Request::builder(crate::Method::Post, url)
                .json(body)?
                .build();
            self.execute(request).await
        }
    }

    /// Execute a PUT request with a JSON body.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization or the request fails.
    fn put_json<T: serde::Serialize + Send + Sync>(
        &self,
        url: &str,
        body: &T,
    ) -> impl Future<Output = Result<Response<Bytes>>> + Send {
        async move {
            let url = url::Url::parse(url)?;
            let request = Request::builder(crate::Method::Put, url)
                .json(body)?
                .build();
            self.execute(request).await
        }
    }

    /// Execute a DELETE request.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails.
    fn delete(&self, url: &str) -> impl Future<Output = Result<Response<Bytes>>> + Send {
        async move {
            let url = url::Url::parse(url)?;
            let request = Request::builder(crate::Method::Delete, url).build();
            self.execute(request).await
        }
    }
}

// Blanket implementation for all HttpClient implementors
impl<T: HttpClient> HttpClientExt for T {}

// ============================================================================
// Pincer Client Trait
// ============================================================================

/// Trait for types that can be used as pincer API clients.
///
/// This trait combines HTTP execution capability with a base URL, enabling
/// the `#[pincer]` macro to generate implementations for any compatible type.
///
/// # Implementing `PincerClient`
///
/// You can implement this trait for your own types to:
/// - Add custom request interceptors (e.g., authentication headers)
/// - Create mock clients for testing
/// - Wrap existing HTTP clients with additional functionality
///
/// # Example
///
/// ```ignore
/// use pincer::{PincerClient, Request, Response, Result};
/// use bytes::Bytes;
/// use url::Url;
///
/// #[derive(Clone)]
/// struct AuthenticatedClient {
///     inner: HyperClient,
///     base_url: Url,
///     token: String,
/// }
///
/// impl PincerClient for AuthenticatedClient {
///     fn execute(
///         &self,
///         request: Request<Bytes>,
///     ) -> impl Future<Output = Result<Response<Bytes>>> + Send {
///         let token = self.token.clone();
///         let inner = self.inner.clone();
///         async move {
///             // Inject auth header
///             let (method, url, mut headers, body) = request.into_parts();
///             headers.insert("Authorization".to_string(), format!("Bearer {}", token));
///             let request = Request::from_parts(method, url, headers, body);
///             inner.execute(request).await
///         }
///     }
///
///     fn base_url(&self) -> &Url {
///         &self.base_url
///     }
/// }
/// ```
pub trait PincerClient: Clone + Send + Sync {
    /// Execute an HTTP request and return the response.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails for any reason:
    /// - Network errors
    /// - TLS errors
    /// - Timeouts
    /// - Invalid response
    fn execute(
        &self,
        request: Request<Bytes>,
    ) -> impl Future<Output = Result<Response<Bytes>>> + Send;

    /// Get the base URL for this client.
    ///
    /// All API paths will be resolved relative to this URL.
    fn base_url(&self) -> &Url;
}

// ============================================================================
// Streaming Client Trait (feature-gated)
// ============================================================================

/// Streaming HTTP client trait.
///
/// This trait extends [`HttpClient`] to support streaming responses.
/// Enable with the `streaming` feature flag.
#[cfg(feature = "streaming")]
pub trait HttpClientStreaming: HttpClient {
    /// Execute an HTTP request and return a streaming response.
    ///
    /// Unlike [`HttpClient::execute`], this method returns a response with
    /// a streaming body that yields chunks as they arrive from the server.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails for any reason:
    /// - Network errors
    /// - TLS errors
    /// - Timeouts
    /// - Invalid response
    fn execute_streaming(
        &self,
        request: Request<Bytes>,
    ) -> impl Future<Output = Result<crate::response::streaming::StreamingResponse>> + Send;
}
