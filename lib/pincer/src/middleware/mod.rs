//! Tower middleware layers for pincer HTTP client.
//!
//! This module provides composable middleware layers that can be applied to
//! the HTTP client using Tower's `Layer` trait. Middleware layers are applied
//! in reverse order - the last layer added is the first to process requests.
//!
//! # Feature Flags
//!
//! Middleware helpers are feature-gated for minimal compile times:
//!
//! | Feature | Description |
//! |---------|-------------|
//! | `middleware-retry` | `.with_retry()` helper |
//! | `middleware-logging` | `.with_logging()` helper |
//! | `middleware-bearer-auth` | `.with_bearer_auth()` helper |
//! | `middleware-basic-auth` | `.with_basic_auth()` helper |
//! | `middleware-concurrency` | `.with_concurrency_limit()` helper |
//! | `middleware-rate-limit` | `.with_rate_limit()` helper |
//! | `middleware-circuit-breaker` | `.with_circuit_breaker()` helper |
//! | `middleware-metrics` | `.with_metrics()` helper |
//! | `middleware-core` | Core middleware bundle |
//! | `middleware-resilience` | Rate limit + circuit breaker |
//! | `middleware-full` | All middleware |
//!
//! # Available Layers
//!
//! ## Custom Layers (pincer)
//!
//! - [`BearerAuthLayer`] - Adds `Authorization: Bearer <token>` header
//! - [`BasicAuthLayer`] - Adds `Authorization: Basic <base64>` header
//! - [`LoggingLayer`] - Logs requests/responses using `tracing`
//! - [`RetryPolicy`] - Configurable retry policy for [`RetryLayer`]
//! - [`RateLimitLayer`] - Limits request rate using token bucket algorithm
//! - [`CircuitBreakerLayer`] - Implements circuit breaker pattern for fault tolerance
//! - [`MetricsLayer`] - Records HTTP metrics (counters, histograms)
//!
//! ## Tower Layers (always available)
//!
//! - [`RetryLayer`] - Retries failed requests based on a policy
//! - [`ConcurrencyLimitLayer`] - Limits concurrent requests
//!
//! # Example: Using the Builder API
//!
//! ```ignore
//! use pincer::HyperClient;
//! use std::time::Duration;
//!
//! // Simple usage with helper methods (feature-gated)
//! let client = HyperClient::builder()
//!     .with_retry(3)
//!     .with_bearer_auth("my-token")
//!     .with_logging()
//!     .build();
//!
//! // Power users: raw layer access (always available)
//! use pincer::middleware::{BearerAuthLayer, ServiceBuilder};
//! let client = HyperClient::builder()
//!     .layer(BearerAuthLayer::new("my-token"))
//!     .build();
//! ```
//!
//! # Note on tower-http
//!
//! tower-http middleware (`TraceLayer`, `FollowRedirectLayer`, etc.) are not directly
//! compatible with pincer's Request/Response types as they work with `http::Request`.
//! For these features, consider using the raw hyper client or implementing custom adapters.

#[cfg(feature = "middleware-basic-auth")]
mod basic_auth;
mod bearer_auth;
#[cfg(feature = "middleware-circuit-breaker")]
mod circuit_breaker;
#[cfg(feature = "middleware-decompression")]
mod decompression;
#[cfg(feature = "middleware-follow-redirect")]
mod follow_redirect;
mod logging;
#[cfg(feature = "middleware-metrics")]
mod metrics;
#[cfg(feature = "middleware-rate-limit")]
mod rate_limit;
mod retry;

// Custom middleware (always available)
#[cfg(feature = "middleware-basic-auth")]
pub use basic_auth::{BasicAuth, BasicAuthLayer};
pub use bearer_auth::{BearerAuth, BearerAuthLayer};
#[cfg(feature = "middleware-circuit-breaker")]
pub use circuit_breaker::{
    CircuitBreaker, CircuitBreakerConfig, CircuitBreakerLayer, CircuitState,
};
#[cfg(feature = "middleware-decompression")]
pub use decompression::{Decompression, DecompressionLayer};
#[cfg(feature = "middleware-follow-redirect")]
pub use follow_redirect::{DEFAULT_MAX_REDIRECTS, FollowRedirect, FollowRedirectLayer};
pub use logging::{LogLevel, Logging, LoggingLayer};
#[cfg(feature = "middleware-metrics")]
pub use metrics::{Metrics, MetricsLayer};
#[cfg(feature = "middleware-rate-limit")]
pub use rate_limit::{RateLimit, RateLimitLayer};
pub use retry::RetryPolicy;

// Re-export tower types for convenience (always available)
pub use tower::{Layer, ServiceBuilder};

// Re-export tower middleware layers (always available)
pub use tower::limit::ConcurrencyLimitLayer;
pub use tower::retry::RetryLayer;
