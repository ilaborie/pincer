//! Rate limiting middleware using governor.
//!
//! This middleware limits the rate of outgoing requests using a token bucket algorithm.

use std::future::Future;
use std::num::NonZeroU32;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use bytes::Bytes;
use governor::{Quota, RateLimiter, clock::DefaultClock, state::InMemoryState};
use tower::{Layer, Service};

use crate::{Error, Request, Response, Result};

/// Type alias for the governor rate limiter.
type GovernorLimiter = RateLimiter<governor::state::NotKeyed, InMemoryState, DefaultClock>;

/// Layer that applies rate limiting to requests.
///
/// Uses a token bucket algorithm via the `governor` crate.
///
/// # Example
///
/// ```ignore
/// use pincer::middleware::RateLimitLayer;
///
/// // Allow 10 requests per second
/// let layer = RateLimitLayer::per_second(10);
///
/// // Allow 100 requests per minute
/// let layer = RateLimitLayer::per_minute(100);
/// ```
#[derive(Debug, Clone)]
pub struct RateLimitLayer {
    limiter: Arc<GovernorLimiter>,
}

impl RateLimitLayer {
    /// Create a rate limiter allowing `count` requests per second.
    ///
    /// # Panics
    ///
    /// Panics if `count` is zero.
    #[must_use]
    #[allow(clippy::expect_used)]
    pub fn per_second(count: u32) -> Self {
        let count = NonZeroU32::new(count).expect("count must be non-zero");
        let quota = Quota::per_second(count);
        Self {
            limiter: Arc::new(RateLimiter::direct(quota)),
        }
    }

    /// Create a rate limiter allowing `count` requests per minute.
    ///
    /// # Panics
    ///
    /// Panics if `count` is zero.
    #[must_use]
    #[allow(clippy::expect_used)]
    pub fn per_minute(count: u32) -> Self {
        let count = NonZeroU32::new(count).expect("count must be non-zero");
        let quota = Quota::per_minute(count);
        Self {
            limiter: Arc::new(RateLimiter::direct(quota)),
        }
    }

    /// Create a rate limiter with a custom quota.
    #[must_use]
    pub fn with_quota(quota: Quota) -> Self {
        Self {
            limiter: Arc::new(RateLimiter::direct(quota)),
        }
    }
}

impl<S> Layer<S> for RateLimitLayer {
    type Service = RateLimit<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RateLimit {
            inner,
            limiter: Arc::clone(&self.limiter),
        }
    }
}

/// Service that applies rate limiting to requests.
#[derive(Debug, Clone)]
pub struct RateLimit<S> {
    inner: S,
    limiter: Arc<GovernorLimiter>,
}

impl<S> RateLimit<S> {
    /// Create a new rate-limited service.
    pub fn new(inner: S, limiter: Arc<GovernorLimiter>) -> Self {
        Self { inner, limiter }
    }
}

impl<S> Service<Request<Bytes>> for RateLimit<S>
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
        let limiter = Arc::clone(&self.limiter);
        let mut inner = self.inner.clone();

        Box::pin(async move {
            // Wait until we're allowed to proceed
            limiter.until_ready().await;

            // Execute the request
            inner.call(request).await
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::pin::Pin;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::time::Instant;

    use tower::{Layer, ServiceExt};

    use super::*;
    use crate::Method;

    /// Mock service that returns configurable responses.
    #[derive(Clone)]
    struct MockService {
        status: u16,
        call_count: Arc<AtomicU32>,
    }

    impl MockService {
        fn new(status: u16) -> Self {
            Self {
                status,
                call_count: Arc::new(AtomicU32::new(0)),
            }
        }

        fn call_count(&self) -> u32 {
            self.call_count.load(Ordering::SeqCst)
        }
    }

    impl Service<Request<Bytes>> for MockService {
        type Response = Response<Bytes>;
        type Error = Error;
        type Future = Pin<Box<dyn std::future::Future<Output = Result<Self::Response>> + Send>>;

        fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<()>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, _request: Request<Bytes>) -> Self::Future {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            let status = self.status;

            Box::pin(async move { Ok(Response::new(status, HashMap::new(), Bytes::new())) })
        }
    }

    fn create_request() -> Request<Bytes> {
        let url = url::Url::parse("https://example.com/test").expect("valid url");
        Request::builder(Method::Get, url).build()
    }

    #[test]
    fn rate_limit_layer_clone() {
        let layer = RateLimitLayer::per_second(10);
        let _cloned = layer.clone();
    }

    #[test]
    fn rate_limit_per_minute() {
        let layer = RateLimitLayer::per_minute(60);
        let _cloned = layer.clone();
    }

    #[test]
    fn rate_limit_with_quota() {
        let quota = Quota::per_second(NonZeroU32::new(5).expect("non-zero"));
        let layer = RateLimitLayer::with_quota(quota);
        let _cloned = layer.clone();
    }

    #[test]
    fn rate_limit_new() {
        let limiter = Arc::new(RateLimiter::direct(Quota::per_second(
            NonZeroU32::new(10).expect("non-zero"),
        )));
        let inner = MockService::new(200);
        let service = RateLimit::new(inner, limiter);
        assert_eq!(service.inner.status, 200);
    }

    #[tokio::test]
    async fn rate_limit_allows_requests() {
        let mock = MockService::new(200);
        let layer = RateLimitLayer::per_second(100);
        let mut service = layer.layer(mock.clone());

        let request = create_request();
        let result = service.ready().await.expect("ready").call(request).await;

        assert!(result.is_ok());
        assert_eq!(result.expect("response").status(), 200);
        assert_eq!(mock.call_count(), 1);
    }

    #[tokio::test]
    async fn rate_limit_multiple_requests() {
        let mock = MockService::new(200);
        let layer = RateLimitLayer::per_second(100);
        let mut service = layer.layer(mock.clone());

        // Make several requests quickly
        for _ in 0..5 {
            let request = create_request();
            let result = service.ready().await.expect("ready").call(request).await;
            assert!(result.is_ok());
        }

        assert_eq!(mock.call_count(), 5);
    }

    #[tokio::test]
    async fn rate_limit_respects_limit() {
        let mock = MockService::new(200);
        // Very restrictive rate: 1 request per second
        let layer = RateLimitLayer::per_second(1);
        let mut service = layer.layer(mock.clone());

        // First request should be immediate
        let start = Instant::now();
        let request = create_request();
        let result = service.ready().await.expect("ready").call(request).await;
        assert!(result.is_ok());

        // Second request should wait
        let request = create_request();
        let result = service.ready().await.expect("ready").call(request).await;
        assert!(result.is_ok());

        let elapsed = start.elapsed();
        // Should have waited approximately 1 second
        assert!(
            elapsed >= std::time::Duration::from_millis(900),
            "rate limiter should have delayed second request, elapsed: {elapsed:?}"
        );

        assert_eq!(mock.call_count(), 2);
    }

    #[tokio::test]
    async fn rate_limit_high_throughput() {
        let mock = MockService::new(200);
        // High rate limit - should process quickly
        let layer = RateLimitLayer::per_second(1000);
        let mut service = layer.layer(mock.clone());

        let start = Instant::now();
        for _ in 0..10 {
            let request = create_request();
            let result = service.ready().await.expect("ready").call(request).await;
            assert!(result.is_ok());
        }
        let elapsed = start.elapsed();

        assert_eq!(mock.call_count(), 10);
        // Should complete quickly (under 100ms)
        assert!(
            elapsed < std::time::Duration::from_millis(100),
            "high throughput requests should complete quickly, elapsed: {elapsed:?}"
        );
    }
}
