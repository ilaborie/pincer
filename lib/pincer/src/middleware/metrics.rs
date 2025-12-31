//! Metrics middleware using the metrics crate facade.
//!
//! This middleware records HTTP request/response metrics using the `metrics` crate,
//! which allows integration with various metrics backends (Prometheus, `StatsD`, etc.).

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Instant;

use bytes::Bytes;
use tower::{Layer, Service};

use crate::{Error, Request, Response, Result};

/// Labels used for metrics.
const LABEL_METHOD: &str = "method";
const LABEL_STATUS: &str = "status";

/// Metric names.
const METRIC_REQUESTS_TOTAL: &str = "http_client_requests_total";
const METRIC_REQUEST_DURATION: &str = "http_client_request_duration_seconds";
const METRIC_REQUESTS_IN_FLIGHT: &str = "http_client_requests_in_flight";

/// Layer that records HTTP metrics.
///
/// Records the following metrics:
/// - `http_client_requests_total` (counter): Total number of requests, labeled by method and status
/// - `http_client_request_duration_seconds` (histogram): Request duration in seconds
/// - `http_client_requests_in_flight` (gauge): Number of requests currently in flight
///
/// # Example
///
/// ```ignore
/// use pincer::middleware::MetricsLayer;
///
/// let layer = MetricsLayer::new();
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct MetricsLayer {
    _private: (),
}

impl MetricsLayer {
    /// Create a new metrics layer.
    #[must_use]
    pub fn new() -> Self {
        Self { _private: () }
    }
}

impl<S> Layer<S> for MetricsLayer {
    type Service = Metrics<S>;

    fn layer(&self, inner: S) -> Self::Service {
        Metrics { inner }
    }
}

/// Service that records HTTP metrics.
#[derive(Debug, Clone)]
pub struct Metrics<S> {
    inner: S,
}

impl<S> Metrics<S> {
    /// Create a new metrics service wrapping the given service.
    pub fn new(inner: S) -> Self {
        Self { inner }
    }
}

impl<S> Service<Request<Bytes>> for Metrics<S>
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
        let method = request.method().to_string();
        let start = Instant::now();
        let mut inner = self.inner.clone();

        // Increment in-flight gauge
        metrics::gauge!(METRIC_REQUESTS_IN_FLIGHT).increment(1.0);

        Box::pin(async move {
            let result = inner.call(request).await;

            // Decrement in-flight gauge
            metrics::gauge!(METRIC_REQUESTS_IN_FLIGHT).decrement(1.0);

            // Record duration
            let duration = start.elapsed().as_secs_f64();
            metrics::histogram!(METRIC_REQUEST_DURATION, LABEL_METHOD => method.clone())
                .record(duration);

            // Record request count with status
            let status = match &result {
                Ok(response) => response.status().to_string(),
                Err(_) => "error".to_string(),
            };

            metrics::counter!(
                METRIC_REQUESTS_TOTAL,
                LABEL_METHOD => method,
                LABEL_STATUS => status
            )
            .increment(1);

            result
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    use tower::{Layer, ServiceExt};

    use super::*;
    use crate::Method;

    /// Mock service that returns configurable responses.
    #[derive(Clone)]
    struct MockService {
        status: u16,
        call_count: Arc<AtomicU32>,
        should_error: bool,
    }

    impl MockService {
        fn new(status: u16) -> Self {
            Self {
                status,
                call_count: Arc::new(AtomicU32::new(0)),
                should_error: false,
            }
        }

        fn with_error() -> Self {
            Self {
                status: 0,
                call_count: Arc::new(AtomicU32::new(0)),
                should_error: true,
            }
        }

        fn call_count(&self) -> u32 {
            self.call_count.load(Ordering::SeqCst)
        }
    }

    impl Service<Request<Bytes>> for MockService {
        type Response = Response<Bytes>;
        type Error = Error;
        type Future = Pin<Box<dyn Future<Output = Result<Self::Response>> + Send>>;

        fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<()>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, _request: Request<Bytes>) -> Self::Future {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            let status = self.status;
            let should_error = self.should_error;

            Box::pin(async move {
                if should_error {
                    Err(Error::connection("mock error"))
                } else {
                    Ok(Response::new(status, HashMap::new(), Bytes::new()))
                }
            })
        }
    }

    fn create_request() -> Request<Bytes> {
        let url = url::Url::parse("https://example.com/test").expect("valid url");
        Request::builder(Method::Get, url).build()
    }

    #[test]
    #[allow(clippy::no_effect_underscore_binding)]
    fn metrics_layer_copy() {
        let layer = MetricsLayer::new();
        let _copied = layer;
        // Verify it was copied, not moved
        let _another = layer;
    }

    #[test]
    fn metrics_layer_default() {
        let _layer = MetricsLayer::default();
    }

    #[tokio::test]
    async fn metrics_service_success() {
        let mock = MockService::new(200);
        let layer = MetricsLayer::new();
        let mut service = layer.layer(mock.clone());

        let request = create_request();
        let result = service.ready().await.expect("ready").call(request).await;

        assert!(result.is_ok());
        assert_eq!(result.expect("response").status(), 200);
        assert_eq!(mock.call_count(), 1);
    }

    #[tokio::test]
    async fn metrics_service_error_response() {
        let mock = MockService::new(500);
        let layer = MetricsLayer::new();
        let mut service = layer.layer(mock.clone());

        let request = create_request();
        let result = service.ready().await.expect("ready").call(request).await;

        assert!(result.is_ok());
        assert_eq!(result.expect("response").status(), 500);
        assert_eq!(mock.call_count(), 1);
    }

    #[tokio::test]
    async fn metrics_service_connection_error() {
        let mock = MockService::with_error();
        let layer = MetricsLayer::new();
        let mut service = layer.layer(mock.clone());

        let request = create_request();
        let result = service.ready().await.expect("ready").call(request).await;

        assert!(result.is_err());
        assert_eq!(mock.call_count(), 1);
    }

    #[tokio::test]
    async fn metrics_service_multiple_requests() {
        let mock = MockService::new(200);
        let layer = MetricsLayer::new();
        let mut service = layer.layer(mock.clone());

        for _ in 0..5 {
            let request = create_request();
            let result = service.ready().await.expect("ready").call(request).await;
            assert!(result.is_ok());
        }

        assert_eq!(mock.call_count(), 5);
    }

    #[test]
    fn metrics_new() {
        let inner = MockService::new(200);
        let service = Metrics::new(inner);
        // Verify service was created
        assert_eq!(service.inner.status, 200);
    }
}
