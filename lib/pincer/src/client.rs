//! HTTP client implementation using hyper-util.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::Duration;

use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper_rustls::HttpsConnector;
use hyper_util::{
    client::legacy::{Client, connect::HttpConnector},
    rt::TokioExecutor,
};
use tower::Layer;
use tower::util::BoxCloneService;
use tower_service::Service;

use crate::{
    Error, Request, Response, Result,
    config::{ClientConfig, ClientConfigBuilder},
    connector::https_connector,
};

// Feature-gated imports for streaming
#[cfg(feature = "streaming")]
use futures_util::TryStreamExt;
#[cfg(feature = "streaming")]
use http_body_util::BodyStream;
#[cfg(feature = "streaming")]
use pincer_core::StreamingBody;

// Feature-gated imports for middleware
#[cfg(feature = "middleware-basic-auth")]
use crate::middleware::BasicAuthLayer;
#[cfg(feature = "middleware-bearer-auth")]
use crate::middleware::BearerAuthLayer;
#[cfg(feature = "middleware-decompression")]
use crate::middleware::DecompressionLayer;
#[cfg(feature = "middleware-follow-redirect")]
use crate::middleware::FollowRedirectLayer;
#[cfg(feature = "middleware-logging")]
use crate::middleware::LoggingLayer;
#[cfg(feature = "middleware-metrics")]
use crate::middleware::MetricsLayer;
#[cfg(feature = "middleware-rate-limit")]
use crate::middleware::RateLimitLayer;
#[cfg(feature = "middleware-retry")]
use crate::middleware::RetryPolicy;
#[cfg(feature = "middleware-circuit-breaker")]
use crate::middleware::{CircuitBreakerConfig, CircuitBreakerLayer};
#[cfg(feature = "middleware-concurrency")]
use tower::limit::ConcurrencyLimitLayer;
#[cfg(feature = "middleware-retry")]
use tower::retry::RetryLayer;

// ============================================================================
// Type-Erased Service for Middleware Composition
// ============================================================================

/// Type-erased service for middleware composition.
///
/// This type allows storing and composing arbitrary Tower layers without
/// exposing complex generic types to users.
pub type BoxedService = BoxCloneService<Request<Bytes>, Response<Bytes>, Error>;

/// Future type for Tower Service implementation.
pub type ServiceFuture = Pin<Box<dyn Future<Output = Result<Response<Bytes>>> + Send + 'static>>;

/// Thread-safe wrapper for `BoxedService`.
///
/// This wrapper uses a Mutex to make the service Sync, which is required
/// by the `HttpClient` trait.
#[derive(Clone)]
struct SyncService {
    inner: Arc<Mutex<BoxedService>>,
}

impl SyncService {
    fn new(service: BoxedService) -> Self {
        Self {
            inner: Arc::new(Mutex::new(service)),
        }
    }

    fn call(&self, request: Request<Bytes>) -> ServiceFuture {
        // Lock, clone the service, and release the lock immediately
        let mut service = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();

        Box::pin(async move { service.call(request).await })
    }
}

// ============================================================================
// Raw Client (internal, used for direct hyper access)
// ============================================================================

/// Raw HTTP client using hyper-util (internal implementation).
#[derive(Clone)]
struct RawHyperClient {
    inner: Client<HttpsConnector<HttpConnector>, Full<Bytes>>,
    config: ClientConfig,
}

impl RawHyperClient {
    fn new(config: ClientConfig) -> Self {
        let connector = https_connector();

        let inner = Client::builder(TokioExecutor::new())
            .pool_idle_timeout(config.pool_idle_timeout)
            .pool_max_idle_per_host(config.pool_idle_per_host)
            .build(connector);

        Self { inner, config }
    }

    /// Build a hyper request from a pincer request.
    fn build_hyper_request(request: Request<Bytes>) -> Result<http::Request<Full<Bytes>>> {
        let (method, url, headers, body, extensions) = request.into_parts();

        let mut builder = http::Request::builder()
            .method(http::Method::from(method))
            .uri(url.as_str());

        for (name, value) in &headers {
            builder = builder.header(name.as_str(), value.as_str());
        }

        let body = body.map_or_else(Full::default, Full::new);
        let mut http_request = builder
            .body(body)
            .map_err(|e| Error::invalid_request(e.to_string()))?;

        // Transfer extensions to the http::Request
        *http_request.extensions_mut() = extensions;

        Ok(http_request)
    }

    /// Extract response headers as a `HashMap`.
    fn extract_headers(headers: &http::HeaderMap) -> HashMap<String, String> {
        headers
            .iter()
            .filter_map(|(name, value)| {
                value
                    .to_str()
                    .ok()
                    .map(|v| (name.to_string(), v.to_string()))
            })
            .collect()
    }

    async fn execute(&self, request: Request<Bytes>) -> Result<Response<Bytes>> {
        let hyper_request = Self::build_hyper_request(request)?;

        let response = tokio::time::timeout(self.config.timeout, self.inner.request(hyper_request))
            .await
            .map_err(|_| Error::Timeout)?
            .map_err(Self::map_hyper_error)?;

        let status = response.status().as_u16();
        let response_headers = Self::extract_headers(response.headers());

        let body = response
            .into_body()
            .collect()
            .await
            .map_err(|e| Error::connection(e.to_string()))?
            .to_bytes();

        Ok(Response::new(status, response_headers, body))
    }

    #[allow(clippy::needless_pass_by_value)]
    fn map_hyper_error(err: hyper_util::client::legacy::Error) -> Error {
        let msg = err.to_string();

        if err.is_connect() {
            return Error::connection(msg);
        }

        if msg.contains("ssl") || msg.contains("tls") || msg.contains("certificate") {
            return Error::tls(msg);
        }

        Error::connection(msg)
    }

    /// Execute a request and return a streaming response.
    #[cfg(feature = "streaming")]
    async fn execute_streaming(
        &self,
        request: Request<Bytes>,
    ) -> Result<pincer_core::StreamingResponse> {
        let hyper_request = Self::build_hyper_request(request)?;

        let response = tokio::time::timeout(self.config.timeout, self.inner.request(hyper_request))
            .await
            .map_err(|_| Error::Timeout)?
            .map_err(Self::map_hyper_error)?;

        let status = response.status().as_u16();
        let response_headers = Self::extract_headers(response.headers());

        let body_stream = BodyStream::new(response.into_body());
        let streaming_body: StreamingBody = Box::pin(
            body_stream
                .map_ok(|frame| frame.into_data().unwrap_or_default())
                .map_err(|e| Error::connection(e.to_string())),
        );

        Ok(pincer_core::StreamingResponse::new(
            status,
            response_headers,
            streaming_body,
        ))
    }
}

impl Service<Request<Bytes>> for RawHyperClient {
    type Response = Response<Bytes>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response>> + Send + 'static>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: Request<Bytes>) -> Self::Future {
        let client = self.clone();
        Box::pin(async move { client.execute(request).await })
    }
}

// ============================================================================
// Public Client
// ============================================================================

/// HTTP client using hyper-util with connection pooling, TLS, and middleware support.
///
/// # Example
///
/// ```ignore
/// use pincer::HyperClient;
/// use std::time::Duration;
///
/// // Simple client without middleware
/// let client = HyperClient::new();
///
/// // Client with middleware (requires feature flags)
/// let client = HyperClient::builder()
///     .with_timeout(Duration::from_secs(30))
///     .with_retry(3)
///     .build();
/// ```
#[derive(Clone)]
pub struct HyperClient {
    service: SyncService,
    config: ClientConfig,
}

impl std::fmt::Debug for HyperClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HyperClient")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl HyperClient {
    /// Create a new client with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(ClientConfig::default())
    }

    /// Create a new client with custom configuration (no middleware).
    #[must_use]
    pub fn with_config(config: ClientConfig) -> Self {
        let raw = RawHyperClient::new(config.clone());
        Self {
            service: SyncService::new(BoxCloneService::new(raw)),
            config,
        }
    }

    /// Create a raw client for internal use by the builder.
    fn with_config_raw(config: ClientConfig) -> RawHyperClient {
        RawHyperClient::new(config)
    }

    /// Create a client with a pre-configured service (used by builder).
    fn with_service(service: BoxedService, config: ClientConfig) -> Self {
        Self {
            service: SyncService::new(service),
            config,
        }
    }

    /// Create a new client builder.
    #[must_use]
    pub fn builder() -> HyperClientBuilder {
        HyperClientBuilder::default()
    }

    /// Get the client configuration.
    #[must_use]
    pub const fn config(&self) -> &ClientConfig {
        &self.config
    }
}

impl Default for HyperClient {
    fn default() -> Self {
        Self::new()
    }
}

impl pincer_core::HttpClient for HyperClient {
    async fn execute(&self, request: Request<Bytes>) -> Result<Response<Bytes>> {
        self.service.call(request).await
    }
}

/// Streaming HTTP client implementation.
///
/// Note: Streaming bypasses middleware since we need to return the raw hyper response
/// body. Middleware is applied to the buffered `execute()` method.
#[cfg(feature = "streaming")]
impl pincer_core::HttpClientStreaming for HyperClient {
    async fn execute_streaming(
        &self,
        request: Request<Bytes>,
    ) -> Result<pincer_core::StreamingResponse> {
        // Create a raw client with the same config to bypass middleware
        let raw_client = RawHyperClient::new(self.config.clone());
        raw_client.execute_streaming(request).await
    }
}

// ============================================================================
// Tower Service Implementation
// ============================================================================

impl Service<Request<Bytes>> for HyperClient {
    type Response = Response<Bytes>;
    type Error = Error;
    type Future = ServiceFuture;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<()>> {
        // SyncService is always ready (the underlying service is polled when called)
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: Request<Bytes>) -> Self::Future {
        self.service.call(request)
    }
}

/// Builder for [`HyperClient`].
///
/// Provides an ergonomic API for configuring the HTTP client with middleware.
///
/// # Example
///
/// ```ignore
/// use pincer::HyperClient;
/// use std::time::Duration;
///
/// // Simple usage with helper methods
/// let client = HyperClient::builder()
///     .with_timeout(Duration::from_secs(30))
///     .with_retry(3)
///     .build();
///
/// // Power users: raw layer access
/// use pincer::middleware::TimeoutLayer;
/// let client = HyperClient::builder()
///     .layer(TimeoutLayer::new(Duration::from_secs(30)))
///     .build();
/// ```
#[derive(Default)]
pub struct HyperClientBuilder {
    config: ClientConfigBuilder,
    layers: Vec<Arc<dyn Fn(BoxedService) -> BoxedService + Send + Sync>>,
    use_defaults: bool,
}

impl std::fmt::Debug for HyperClientBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HyperClientBuilder")
            .field("config", &self.config)
            .field("layers_count", &self.layers.len())
            .field("use_defaults", &self.use_defaults)
            .finish()
    }
}

impl HyperClientBuilder {
    // ========================================================================
    // Core Configuration
    // ========================================================================

    /// Set the request timeout (applied at the connection level, not middleware).
    #[must_use]
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.config = self.config.timeout(timeout);
        self
    }

    /// Set the connection timeout.
    #[must_use]
    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.config = self.config.connect_timeout(timeout);
        self
    }

    /// Set the maximum idle connections per host.
    #[must_use]
    pub fn pool_idle_per_host(mut self, count: usize) -> Self {
        self.config = self.config.pool_idle_per_host(count);
        self
    }

    /// Set the idle connection timeout.
    #[must_use]
    pub fn pool_idle_timeout(mut self, timeout: Duration) -> Self {
        self.config = self.config.pool_idle_timeout(timeout);
        self
    }

    // ========================================================================
    // Generic Middleware API (always available)
    // ========================================================================

    /// Add a Tower layer to the client.
    ///
    /// Layers are applied in order: first added = outermost (processes requests first).
    ///
    /// # Example
    ///
    /// ```ignore
    /// use pincer::HyperClient;
    /// use pincer::middleware::TimeoutLayer;
    /// use std::time::Duration;
    ///
    /// let client = HyperClient::builder()
    ///     .layer(TimeoutLayer::new(Duration::from_secs(30)))
    ///     .build();
    /// ```
    #[must_use]
    pub fn layer<L>(mut self, layer: L) -> Self
    where
        L: Layer<BoxedService> + Send + Sync + 'static,
        L::Service: Service<Request<Bytes>, Response = Response<Bytes>, Error = Error>
            + Clone
            + Send
            + 'static,
        <L::Service as Service<Request<Bytes>>>::Future: Send,
    {
        self.layers.push(Arc::new(move |service| {
            BoxCloneService::new(layer.layer(service))
        }));
        self
    }

    /// Add middleware using the reqwest-middleware style `.with()` method.
    ///
    /// This is an alias for `.layer()` for users familiar with reqwest-middleware.
    #[must_use]
    pub fn with<L>(self, layer: L) -> Self
    where
        L: Layer<BoxedService> + Send + Sync + 'static,
        L::Service: Service<Request<Bytes>, Response = Response<Bytes>, Error = Error>
            + Clone
            + Send
            + 'static,
        <L::Service as Service<Request<Bytes>>>::Future: Send,
    {
        self.layer(layer)
    }

    // ========================================================================
    // Defaults Control
    // ========================================================================

    /// Enable sensible default middleware.
    ///
    /// Currently includes:
    /// - Logging (if `middleware-logging` feature is enabled)
    /// - Follow redirects (if `tower-http-follow-redirect` feature is enabled)
    ///
    /// Defaults are applied before any layers added via `.layer()`.
    #[must_use]
    pub fn with_defaults(mut self) -> Self {
        self.use_defaults = true;
        self
    }

    /// Disable all default middleware.
    #[must_use]
    pub fn without_defaults(mut self) -> Self {
        self.use_defaults = false;
        self
    }

    // ========================================================================
    // Feature-Gated Helper Methods
    // ========================================================================

    /// Add retry middleware with the given number of retries.
    ///
    /// Uses the default retry policy: retries on 5xx, 429, connection errors, timeouts.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let client = HyperClient::builder()
    ///     .with_retry(3)
    ///     .build();
    /// ```
    #[cfg(feature = "middleware-retry")]
    #[must_use]
    pub fn with_retry(self, max_retries: u32) -> Self {
        self.layer(RetryLayer::new(RetryPolicy::new(max_retries)))
    }

    /// Add bearer token authentication.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let client = HyperClient::builder()
    ///     .with_bearer_auth("my-secret-token")
    ///     .build();
    /// ```
    #[cfg(feature = "middleware-bearer-auth")]
    #[must_use]
    pub fn with_bearer_auth(self, token: impl Into<String>) -> Self {
        self.layer(BearerAuthLayer::new(token))
    }

    /// Add basic authentication.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let client = HyperClient::builder()
    ///     .with_basic_auth("username", "password")
    ///     .build();
    /// ```
    #[cfg(feature = "middleware-basic-auth")]
    #[must_use]
    pub fn with_basic_auth(self, username: impl AsRef<str>, password: impl AsRef<str>) -> Self {
        self.layer(BasicAuthLayer::new(username, password))
    }

    /// Add request/response logging.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let client = HyperClient::builder()
    ///     .with_logging()
    ///     .build();
    /// ```
    #[cfg(feature = "middleware-logging")]
    #[must_use]
    pub fn with_logging(self) -> Self {
        self.layer(LoggingLayer::new())
    }

    /// Add debug-level logging (includes headers and more detail).
    #[cfg(feature = "middleware-logging")]
    #[must_use]
    pub fn with_debug_logging(self) -> Self {
        self.layer(LoggingLayer::debug())
    }

    /// Add concurrency limiting.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let client = HyperClient::builder()
    ///     .with_concurrency_limit(10)
    ///     .build();
    /// ```
    #[cfg(feature = "middleware-concurrency")]
    #[must_use]
    pub fn with_concurrency_limit(self, max: usize) -> Self {
        self.layer(ConcurrencyLimitLayer::new(max))
    }

    /// Add rate limiting (requests per second).
    ///
    /// # Example
    ///
    /// ```ignore
    /// let client = HyperClient::builder()
    ///     .with_rate_limit_per_second(10)
    ///     .build();
    /// ```
    #[cfg(feature = "middleware-rate-limit")]
    #[must_use]
    pub fn with_rate_limit_per_second(self, count: u32) -> Self {
        self.layer(RateLimitLayer::per_second(count))
    }

    /// Add rate limiting (requests per minute).
    ///
    /// # Example
    ///
    /// ```ignore
    /// let client = HyperClient::builder()
    ///     .with_rate_limit_per_minute(100)
    ///     .build();
    /// ```
    #[cfg(feature = "middleware-rate-limit")]
    #[must_use]
    pub fn with_rate_limit_per_minute(self, count: u32) -> Self {
        self.layer(RateLimitLayer::per_minute(count))
    }

    /// Add circuit breaker with default configuration.
    ///
    /// Default: 5 failures to open, 30s open duration, 2 successes to close.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let client = HyperClient::builder()
    ///     .with_circuit_breaker()
    ///     .build();
    /// ```
    #[cfg(feature = "middleware-circuit-breaker")]
    #[must_use]
    pub fn with_circuit_breaker(self) -> Self {
        self.layer(CircuitBreakerLayer::new(CircuitBreakerConfig::default()))
    }

    /// Add circuit breaker with custom configuration.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use pincer::middleware::CircuitBreakerConfig;
    /// use std::time::Duration;
    ///
    /// let config = CircuitBreakerConfig::default()
    ///     .with_failure_threshold(3)
    ///     .with_open_duration(Duration::from_secs(60));
    ///
    /// let client = HyperClient::builder()
    ///     .with_circuit_breaker_config(config)
    ///     .build();
    /// ```
    #[cfg(feature = "middleware-circuit-breaker")]
    #[must_use]
    pub fn with_circuit_breaker_config(
        self,
        config: crate::middleware::CircuitBreakerConfig,
    ) -> Self {
        self.layer(CircuitBreakerLayer::new(config))
    }

    /// Add metrics recording.
    ///
    /// Records the following metrics:
    /// - `http_client_requests_total`: Counter by method and status
    /// - `http_client_request_duration_seconds`: Histogram by method
    /// - `http_client_requests_in_flight`: Gauge
    ///
    /// # Example
    ///
    /// ```ignore
    /// let client = HyperClient::builder()
    ///     .with_metrics()
    ///     .build();
    /// ```
    #[cfg(feature = "middleware-metrics")]
    #[must_use]
    pub fn with_metrics(self) -> Self {
        self.layer(MetricsLayer::new())
    }

    /// Add follow redirect middleware.
    ///
    /// This middleware automatically follows HTTP redirects (301, 302, 303, 307, 308).
    /// By default, follows up to 10 redirects.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let client = HyperClient::builder()
    ///     .with_follow_redirects()
    ///     .build();
    /// ```
    #[cfg(feature = "middleware-follow-redirect")]
    #[must_use]
    pub fn with_follow_redirects(self) -> Self {
        self.layer(FollowRedirectLayer::new())
    }

    /// Add follow redirect middleware with a custom maximum redirect count.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let client = HyperClient::builder()
    ///     .with_follow_redirects_max(5)
    ///     .build();
    /// ```
    #[cfg(feature = "middleware-follow-redirect")]
    #[must_use]
    pub fn with_follow_redirects_max(self, max_redirects: usize) -> Self {
        self.layer(FollowRedirectLayer::with_max_redirects(max_redirects))
    }

    /// Add automatic response decompression middleware.
    ///
    /// This middleware adds the `Accept-Encoding` header to requests and
    /// automatically decompresses responses encoded with gzip, deflate,
    /// brotli, or zstd.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let client = HyperClient::builder()
    ///     .with_decompression()
    ///     .build();
    /// ```
    #[cfg(feature = "middleware-decompression")]
    #[must_use]
    pub fn with_decompression(self) -> Self {
        self.layer(DecompressionLayer::new())
    }

    // ========================================================================
    // Build
    // ========================================================================

    /// Build the client with all configured middleware.
    #[must_use]
    pub fn build(self) -> HyperClient {
        let config = self.config.build();
        let base_client = HyperClient::with_config_raw(config.clone());

        // Start with base service
        let mut service: BoxedService = BoxCloneService::new(base_client);

        // Apply default layers if enabled
        if self.use_defaults {
            #[cfg(feature = "middleware-logging")]
            {
                service = BoxCloneService::new(LoggingLayer::new().layer(service));
            }
        }

        // Apply user layers in order (first added = outermost)
        for layer_fn in self.layers {
            service = layer_fn(service);
        }

        HyperClient::with_service(service, config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_default() {
        let client = HyperClient::new();
        assert_eq!(client.config().timeout, std::time::Duration::from_secs(30));
    }

    #[test]
    fn client_builder() {
        let client = HyperClient::builder()
            .timeout(std::time::Duration::from_secs(60))
            .pool_idle_per_host(16)
            .build();

        assert_eq!(client.config().timeout, std::time::Duration::from_secs(60));
        assert_eq!(client.config().pool_idle_per_host, 16);
    }

    #[test]
    fn client_is_clone() {
        let client = HyperClient::new();
        let _cloned = client.clone();
    }

    #[test]
    fn client_is_debug() {
        let client = HyperClient::new();
        let debug = format!("{client:?}");
        assert!(debug.contains("HyperClient"));
    }
}
