//! Circuit breaker middleware for fault tolerance.
//!
//! Implements the circuit breaker pattern to prevent cascading failures
//! when a downstream service is experiencing issues.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::task::{Context, Poll};
use std::time::Duration;

use bytes::Bytes;
use tower::{Layer, Service};

use crate::{Error, Request, Response, Result};

/// Circuit breaker states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Circuit is closed, requests flow normally.
    Closed,
    /// Circuit is open, requests are rejected immediately.
    Open,
    /// Circuit is half-open, allowing a limited number of test requests.
    HalfOpen,
}

/// Configuration for the circuit breaker.
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Number of consecutive failures before opening the circuit.
    pub failure_threshold: u32,
    /// Duration the circuit stays open before transitioning to half-open.
    pub open_duration: Duration,
    /// Number of successful requests needed to close the circuit from half-open.
    pub success_threshold: u32,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            open_duration: Duration::from_secs(30),
            success_threshold: 2,
        }
    }
}

impl CircuitBreakerConfig {
    /// Create a new circuit breaker configuration.
    #[must_use]
    pub fn new(failure_threshold: u32, open_duration: Duration, success_threshold: u32) -> Self {
        Self {
            failure_threshold,
            open_duration,
            success_threshold,
        }
    }

    /// Set the failure threshold.
    #[must_use]
    pub const fn with_failure_threshold(mut self, threshold: u32) -> Self {
        self.failure_threshold = threshold;
        self
    }

    /// Set the open duration.
    #[must_use]
    pub const fn with_open_duration(mut self, duration: Duration) -> Self {
        self.open_duration = duration;
        self
    }

    /// Set the success threshold.
    #[must_use]
    pub const fn with_success_threshold(mut self, threshold: u32) -> Self {
        self.success_threshold = threshold;
        self
    }
}

/// Shared state for the circuit breaker.
#[derive(Debug)]
struct CircuitBreakerState {
    /// Current state (0 = Closed, 1 = Open, 2 = `HalfOpen`).
    state: AtomicU32,
    /// Consecutive failure count.
    failure_count: AtomicU32,
    /// Consecutive success count (used in half-open state).
    success_count: AtomicU32,
    /// Timestamp when circuit opened (millis since epoch).
    opened_at: AtomicU64,
    /// Configuration.
    config: CircuitBreakerConfig,
}

impl CircuitBreakerState {
    fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            state: AtomicU32::new(0), // Closed
            failure_count: AtomicU32::new(0),
            success_count: AtomicU32::new(0),
            opened_at: AtomicU64::new(0),
            config,
        }
    }

    fn get_state(&self) -> CircuitState {
        match self.state.load(Ordering::SeqCst) {
            1 => CircuitState::Open,
            2 => CircuitState::HalfOpen,
            _ => CircuitState::Closed,
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    fn current_time_millis() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }

    fn should_allow_request(&self) -> bool {
        match self.get_state() {
            CircuitState::Closed | CircuitState::HalfOpen => true,
            CircuitState::Open => {
                // Check if we should transition to half-open
                let opened_at = self.opened_at.load(Ordering::SeqCst);
                let now = Self::current_time_millis();
                let elapsed = Duration::from_millis(now.saturating_sub(opened_at));

                if elapsed >= self.config.open_duration {
                    // Transition to half-open
                    self.state.store(2, Ordering::SeqCst);
                    self.success_count.store(0, Ordering::SeqCst);
                    true
                } else {
                    false
                }
            }
        }
    }

    fn record_success(&self) {
        match self.get_state() {
            CircuitState::Closed => {
                // Reset failure count on success
                self.failure_count.store(0, Ordering::SeqCst);
            }
            CircuitState::HalfOpen => {
                let count = self.success_count.fetch_add(1, Ordering::SeqCst) + 1;
                if count >= self.config.success_threshold {
                    // Transition to closed
                    self.state.store(0, Ordering::SeqCst);
                    self.failure_count.store(0, Ordering::SeqCst);
                }
            }
            CircuitState::Open => {}
        }
    }

    fn record_failure(&self) {
        match self.get_state() {
            CircuitState::Closed => {
                let count = self.failure_count.fetch_add(1, Ordering::SeqCst) + 1;
                if count >= self.config.failure_threshold {
                    // Transition to open
                    self.state.store(1, Ordering::SeqCst);
                    self.opened_at
                        .store(Self::current_time_millis(), Ordering::SeqCst);
                }
            }
            CircuitState::HalfOpen => {
                // Any failure in half-open returns to open
                self.state.store(1, Ordering::SeqCst);
                self.opened_at
                    .store(Self::current_time_millis(), Ordering::SeqCst);
            }
            CircuitState::Open => {}
        }
    }
}

/// Layer that applies circuit breaker pattern to requests.
///
/// # Example
///
/// ```ignore
/// use pincer::middleware::{CircuitBreakerLayer, CircuitBreakerConfig};
/// use std::time::Duration;
///
/// // Use default configuration
/// let layer = CircuitBreakerLayer::new(CircuitBreakerConfig::default());
///
/// // Custom configuration
/// let config = CircuitBreakerConfig::default()
///     .with_failure_threshold(3)
///     .with_open_duration(Duration::from_secs(60))
///     .with_success_threshold(2);
/// let layer = CircuitBreakerLayer::new(config);
/// ```
#[derive(Debug, Clone)]
pub struct CircuitBreakerLayer {
    state: Arc<CircuitBreakerState>,
}

impl CircuitBreakerLayer {
    /// Create a new circuit breaker layer with the given configuration.
    #[must_use]
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            state: Arc::new(CircuitBreakerState::new(config)),
        }
    }
}

impl<S> Layer<S> for CircuitBreakerLayer {
    type Service = CircuitBreaker<S>;

    fn layer(&self, inner: S) -> Self::Service {
        CircuitBreaker {
            inner,
            state: Arc::clone(&self.state),
        }
    }
}

/// Service that applies circuit breaker pattern to requests.
#[derive(Debug, Clone)]
pub struct CircuitBreaker<S> {
    inner: S,
    state: Arc<CircuitBreakerState>,
}

impl<S> CircuitBreaker<S> {
    /// Get the current circuit state.
    #[must_use]
    pub fn circuit_state(&self) -> CircuitState {
        self.state.get_state()
    }
}

impl<S> Service<Request<Bytes>> for CircuitBreaker<S>
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
        let state = Arc::clone(&self.state);

        // Check if we should allow the request
        if !state.should_allow_request() {
            return Box::pin(async move { Err(Error::connection("circuit breaker is open")) });
        }

        let mut inner = self.inner.clone();

        Box::pin(async move {
            let result = inner.call(request).await;

            match &result {
                Ok(response) => {
                    // Consider 5xx as failures for circuit breaker
                    if response.is_server_error() {
                        state.record_failure();
                    } else {
                        state.record_success();
                    }
                }
                Err(_) => {
                    state.record_failure();
                }
            }

            result
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::pin::Pin;
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
    fn circuit_breaker_config_default() {
        let config = CircuitBreakerConfig::default();
        assert_eq!(config.failure_threshold, 5);
        assert_eq!(config.open_duration, Duration::from_secs(30));
        assert_eq!(config.success_threshold, 2);
    }

    #[test]
    fn circuit_breaker_config_builder() {
        let config = CircuitBreakerConfig::default()
            .with_failure_threshold(10)
            .with_open_duration(Duration::from_secs(60))
            .with_success_threshold(3);

        assert_eq!(config.failure_threshold, 10);
        assert_eq!(config.open_duration, Duration::from_secs(60));
        assert_eq!(config.success_threshold, 3);
    }

    #[test]
    fn circuit_breaker_config_new() {
        let config = CircuitBreakerConfig::new(3, Duration::from_secs(10), 1);
        assert_eq!(config.failure_threshold, 3);
        assert_eq!(config.open_duration, Duration::from_secs(10));
        assert_eq!(config.success_threshold, 1);
    }

    #[test]
    fn circuit_breaker_layer_clone() {
        let layer = CircuitBreakerLayer::new(CircuitBreakerConfig::default());
        let _cloned = layer.clone();
    }

    #[test]
    fn circuit_state_eq() {
        assert_eq!(CircuitState::Closed, CircuitState::Closed);
        assert_eq!(CircuitState::Open, CircuitState::Open);
        assert_eq!(CircuitState::HalfOpen, CircuitState::HalfOpen);
        assert_ne!(CircuitState::Closed, CircuitState::Open);
    }

    #[tokio::test]
    async fn circuit_breaker_starts_closed() {
        let mock = MockService::new(200);
        let config = CircuitBreakerConfig::default();
        let layer = CircuitBreakerLayer::new(config);
        let service = layer.layer(mock);

        assert_eq!(service.circuit_state(), CircuitState::Closed);
    }

    #[tokio::test]
    async fn circuit_breaker_success_stays_closed() {
        let mock = MockService::new(200);
        let config = CircuitBreakerConfig::default().with_failure_threshold(3);
        let layer = CircuitBreakerLayer::new(config);
        let mut service = layer.layer(mock.clone());

        // Multiple successful requests should keep circuit closed
        for _ in 0..5 {
            let request = create_request();
            let result = service.ready().await.expect("ready").call(request).await;
            assert!(result.is_ok());
            assert_eq!(service.circuit_state(), CircuitState::Closed);
        }

        assert_eq!(mock.call_count(), 5);
    }

    #[tokio::test]
    async fn circuit_breaker_opens_after_failures() {
        let mock = MockService::with_error();
        let config = CircuitBreakerConfig::default()
            .with_failure_threshold(3)
            .with_open_duration(Duration::from_secs(60));
        let layer = CircuitBreakerLayer::new(config);
        let mut service = layer.layer(mock.clone());

        // First 3 failures should open the circuit
        for i in 0..3 {
            let request = create_request();
            let result = service.ready().await.expect("ready").call(request).await;
            assert!(result.is_err(), "request {i} should fail");
        }

        assert_eq!(service.circuit_state(), CircuitState::Open);
        assert_eq!(mock.call_count(), 3);

        // Next request should be rejected without calling inner service
        let request = create_request();
        let result = service.ready().await.expect("ready").call(request).await;
        assert!(result.is_err());
        // Mock should not have been called
        assert_eq!(mock.call_count(), 3);
    }

    #[tokio::test]
    async fn circuit_breaker_opens_on_5xx_responses() {
        let mock = MockService::new(500);
        let config = CircuitBreakerConfig::default()
            .with_failure_threshold(2)
            .with_open_duration(Duration::from_secs(60));
        let layer = CircuitBreakerLayer::new(config);
        let mut service = layer.layer(mock.clone());

        // 5xx responses count as failures
        for _ in 0..2 {
            let request = create_request();
            let result = service.ready().await.expect("ready").call(request).await;
            assert!(result.is_ok()); // Response received, but it's a 5xx
        }

        assert_eq!(service.circuit_state(), CircuitState::Open);
    }

    #[tokio::test]
    async fn circuit_breaker_transitions_to_half_open() {
        let mock = MockService::with_error();
        let config = CircuitBreakerConfig::default()
            .with_failure_threshold(1)
            .with_open_duration(Duration::from_millis(50));
        let layer = CircuitBreakerLayer::new(config);
        let mut service = layer.layer(mock);

        // Trigger failure to open circuit
        let request = create_request();
        let _ = service.ready().await.expect("ready").call(request).await;
        assert_eq!(service.circuit_state(), CircuitState::Open);

        // Wait for open duration to pass
        tokio::time::sleep(Duration::from_millis(60)).await;

        // Next request should transition to half-open
        let request = create_request();
        let _ = service.ready().await.expect("ready").call(request).await;
        // After the call, circuit may be open again due to failure in half-open
        // But it transitioned through half-open
    }

    #[tokio::test]
    async fn circuit_breaker_closes_from_half_open_on_success() {
        // Use a mock that can switch between success and failure
        #[derive(Clone)]
        struct SwitchableMock {
            fail_count: Arc<AtomicU32>,
            max_failures: u32,
        }

        impl SwitchableMock {
            fn new(max_failures: u32) -> Self {
                Self {
                    fail_count: Arc::new(AtomicU32::new(0)),
                    max_failures,
                }
            }
        }

        impl Service<Request<Bytes>> for SwitchableMock {
            type Response = Response<Bytes>;
            type Error = Error;
            type Future = Pin<Box<dyn Future<Output = Result<Self::Response>> + Send>>;

            fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<()>> {
                Poll::Ready(Ok(()))
            }

            fn call(&mut self, _request: Request<Bytes>) -> Self::Future {
                let count = self.fail_count.fetch_add(1, Ordering::SeqCst);
                let should_fail = count < self.max_failures;

                Box::pin(async move {
                    if should_fail {
                        Err(Error::connection("mock error"))
                    } else {
                        Ok(Response::new(200, HashMap::new(), Bytes::new()))
                    }
                })
            }
        }

        let mock = SwitchableMock::new(1); // Fail once, then succeed
        let config = CircuitBreakerConfig::default()
            .with_failure_threshold(1)
            .with_open_duration(Duration::from_millis(10))
            .with_success_threshold(1);
        let layer = CircuitBreakerLayer::new(config);
        let mut service = layer.layer(mock);

        // Trigger failure to open circuit
        let request = create_request();
        let _ = service.ready().await.expect("ready").call(request).await;
        assert_eq!(service.circuit_state(), CircuitState::Open);

        // Wait for open duration
        tokio::time::sleep(Duration::from_millis(20)).await;

        // Next request should succeed and close the circuit
        let request = create_request();
        let result = service.ready().await.expect("ready").call(request).await;
        assert!(result.is_ok());
        assert_eq!(service.circuit_state(), CircuitState::Closed);
    }

    #[tokio::test]
    async fn circuit_breaker_success_resets_failure_count() {
        // Mock that alternates between failure and success
        #[derive(Clone)]
        struct AlternatingMock {
            call_count: Arc<AtomicU32>,
        }

        impl Service<Request<Bytes>> for AlternatingMock {
            type Response = Response<Bytes>;
            type Error = Error;
            type Future = Pin<Box<dyn Future<Output = Result<Self::Response>> + Send>>;

            fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<()>> {
                Poll::Ready(Ok(()))
            }

            fn call(&mut self, _request: Request<Bytes>) -> Self::Future {
                let count = self.call_count.fetch_add(1, Ordering::SeqCst);
                // Fail on even calls, succeed on odd
                let should_fail = count.is_multiple_of(2);

                Box::pin(async move {
                    if should_fail {
                        Err(Error::connection("mock error"))
                    } else {
                        Ok(Response::new(200, HashMap::new(), Bytes::new()))
                    }
                })
            }
        }

        let mock = AlternatingMock {
            call_count: Arc::new(AtomicU32::new(0)),
        };
        let config = CircuitBreakerConfig::default().with_failure_threshold(3);
        let layer = CircuitBreakerLayer::new(config);
        let mut service = layer.layer(mock);

        // Alternating fail/success should never open circuit
        // because success resets the failure count
        for _ in 0..10 {
            let request = create_request();
            let _ = service.ready().await.expect("ready").call(request).await;
            assert_eq!(
                service.circuit_state(),
                CircuitState::Closed,
                "circuit should stay closed with alternating results"
            );
        }
    }
}
