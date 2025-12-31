//! Basic authentication middleware.
//!
//! This middleware automatically adds an `Authorization: Basic <base64(user:pass)>` header
//! to all outgoing requests.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use base64::Engine;
use bytes::Bytes;
use tower::{Layer, Service};

use crate::{Error, Request, Response, Result};

/// Layer that adds basic authentication to requests.
///
/// # Example
///
/// ```ignore
/// use pincer::middleware::BasicAuthLayer;
/// use tower::ServiceBuilder;
///
/// let service = ServiceBuilder::new()
///     .layer(BasicAuthLayer::new("username", "password"))
///     .service(client);
/// ```
#[derive(Debug, Clone)]
pub struct BasicAuthLayer {
    /// Base64-encoded "username:password".
    encoded_credentials: Arc<str>,
}

impl BasicAuthLayer {
    /// Create a new basic auth layer with the given username and password.
    pub fn new(username: impl AsRef<str>, password: impl AsRef<str>) -> Self {
        let credentials = format!("{}:{}", username.as_ref(), password.as_ref());
        let encoded = base64::engine::general_purpose::STANDARD.encode(credentials);
        Self {
            encoded_credentials: Arc::from(encoded),
        }
    }
}

impl<S> Layer<S> for BasicAuthLayer {
    type Service = BasicAuth<S>;

    fn layer(&self, inner: S) -> Self::Service {
        BasicAuth {
            inner,
            encoded_credentials: Arc::clone(&self.encoded_credentials),
        }
    }
}

/// Service that adds basic authentication to requests.
#[derive(Debug, Clone)]
pub struct BasicAuth<S> {
    inner: S,
    /// Base64-encoded "username:password".
    encoded_credentials: Arc<str>,
}

impl<S> BasicAuth<S> {
    /// Create a new basic auth service wrapping the given service.
    pub fn new(inner: S, username: impl AsRef<str>, password: impl AsRef<str>) -> Self {
        let credentials = format!("{}:{}", username.as_ref(), password.as_ref());
        let encoded = base64::engine::general_purpose::STANDARD.encode(credentials);
        Self {
            inner,
            encoded_credentials: Arc::from(encoded),
        }
    }
}

impl<S> Service<Request<Bytes>> for BasicAuth<S>
where
    S: Service<Request<Bytes>, Response = Response<Bytes>, Error = Error> + Clone + Send + 'static,
    S::Future: Send,
{
    type Response = Response<Bytes>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<()>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: Request<Bytes>) -> Self::Future {
        // Add the Authorization header
        let (method, url, mut headers, body) = request.into_parts();
        headers.insert(
            "Authorization".to_string(),
            format!("Basic {}", self.encoded_credentials),
        );

        let new_request = Request::builder(method, url)
            .headers(headers)
            .body(body.unwrap_or_default())
            .build();

        let mut inner = self.inner.clone();
        Box::pin(async move { inner.call(new_request).await })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_auth_layer_clone() {
        let layer = BasicAuthLayer::new("user", "pass");
        let _cloned = layer.clone();
    }

    #[test]
    fn basic_auth_encodes_correctly() {
        // "user:pass" -> "dXNlcjpwYXNz"
        let layer = BasicAuthLayer::new("user", "pass");
        assert_eq!(&*layer.encoded_credentials, "dXNlcjpwYXNz");
    }
}
