//! # Chapter 3: Middleware
//!
//! Add cross-cutting concerns like retry, authentication, and logging.
//!
//! ## Feature Flags
//!
//! Enable middleware helpers in `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! pincer = { version = "0.1", features = ["middleware-core"] }
//! ```
//!
//! ## Adding Middleware
//!
//! Use the builder's `configure_client` method:
//!
//! ```ignore
//! let client = UserApiClientBuilder::default()
//!     .configure_client(|builder| {
//!         builder
//!             .with_retry(3)
//!             .with_logging()
//!             .with_bearer_auth("my-token")
//!     })
//!     .build()?;
//! ```
//!
//! ## Available Middleware
//!
//! | Feature | Method | Description |
//! |---------|--------|-------------|
//! | `middleware-retry` | `.with_retry(n)` | Retry failed requests |
//! | `middleware-logging` | `.with_logging()` | Log requests/responses |
//! | `middleware-bearer-auth` | `.with_bearer_auth(token)` | Add Bearer token |
//! | `middleware-concurrency` | `.with_concurrency_limit(n)` | Limit concurrent requests |
//!
//! ## Middleware Order
//!
//! Middleware wraps in the order added. For a request:
//!
//! ```text
//! Request → Retry → Logging → Auth → HTTP
//! Response ← Retry ← Logging ← Auth ← HTTP
//! ```
//!
//! Typical order:
//! 1. Retry (outermost - sees all responses)
//! 2. Logging (logs retries)
//! 3. Auth (innermost - added to every request)
//!
//! ## Using Tower Layers Directly
//!
//! For full control, use Tower layers:
//!
//! ```ignore
//! use tower::ServiceBuilder;
//! use pincer::middleware::RetryLayer;
//!
//! let client = UserApiClientBuilder::default()
//!     .configure_client(|builder| {
//!         builder.with_layer(RetryLayer::new(3))
//!     })
//!     .build()?;
//! ```
//!
//! ## Tower-HTTP Integration
//!
//! Enable additional middleware from tower-http:
//!
//! ```toml
//! pincer = { version = "0.1", features = ["tower-http-recommended"] }
//! ```
//!
//! Available features:
//! - `tower-http-trace` - Request/response tracing
//! - `tower-http-follow-redirect` - Follow redirects
//! - `tower-http-compression` - Compression support
//!
//! ## Custom Middleware
//!
//! Implement the Tower `Layer` and `Service` traits.
//! See the [Tower documentation](https://docs.rs/tower) for details.
//!
//! ## Summary
//!
//! - Use feature flags to enable middleware helpers
//! - Configure via `configure_client` builder method
//! - Order matters: outermost middleware sees final response
//! - Use Tower layers for advanced customization
