//! Bearer token authentication middleware.
//!
//! This middleware automatically adds an `Authorization: Bearer <token>` header
//! to all outgoing requests.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use bytes::Bytes;
use tower::{Layer, Service};

use crate::{Error, Request, Response, Result};

/// Layer that adds bearer token authentication to requests.
///
/// # Example
///
/// ```ignore
/// use pincer::middleware::BearerAuthLayer;
/// use tower::ServiceBuilder;
///
/// let service = ServiceBuilder::new()
///     .layer(BearerAuthLayer::new("my-secret-token"))
///     .service(client);
/// ```
#[derive(Debug, Clone)]
pub struct BearerAuthLayer {
    token: Arc<str>,
}

impl BearerAuthLayer {
    /// Create a new bearer auth layer with the given token.
    pub fn new(token: impl Into<String>) -> Self {
        Self {
            token: Arc::from(token.into()),
        }
    }
}

impl<S> Layer<S> for BearerAuthLayer {
    type Service = BearerAuth<S>;

    fn layer(&self, inner: S) -> Self::Service {
        BearerAuth {
            inner,
            token: Arc::clone(&self.token),
        }
    }
}

/// Service that adds bearer token authentication to requests.
#[derive(Debug, Clone)]
pub struct BearerAuth<S> {
    inner: S,
    token: Arc<str>,
}

impl<S> BearerAuth<S> {
    /// Create a new bearer auth service wrapping the given service.
    pub fn new(inner: S, token: impl Into<String>) -> Self {
        Self {
            inner,
            token: Arc::from(token.into()),
        }
    }
}

impl<S> Service<Request<Bytes>> for BearerAuth<S>
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

    fn call(&mut self, mut request: Request<Bytes>) -> Self::Future {
        request.headers_mut().insert(
            "Authorization".to_string(),
            format!("Bearer {}", self.token),
        );

        let mut inner = self.inner.clone();
        Box::pin(async move { inner.call(request).await })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bearer_auth_layer_clone() {
        let layer = BearerAuthLayer::new("test-token");
        let _cloned = layer.clone();
    }
}
