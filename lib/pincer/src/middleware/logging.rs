//! Request/response logging middleware.
//!
//! This middleware logs HTTP requests and responses using the `tracing` crate.

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Instant;

use bytes::Bytes;
use tower::{Layer, Service};
use tracing::{Instrument, Level, debug, info, span, warn};

use crate::{Error, Request, Response, Result};

/// Layer that adds request/response logging.
///
/// # Example
///
/// ```ignore
/// use pincer::middleware::LoggingLayer;
/// use tower::ServiceBuilder;
///
/// let service = ServiceBuilder::new()
///     .layer(LoggingLayer::new())
///     .service(client);
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct LoggingLayer {
    level: LogLevel,
}

/// Log level for the logging middleware.
#[derive(Debug, Clone, Copy, Default)]
pub enum LogLevel {
    /// Log at debug level (request/response details).
    Debug,
    /// Log at info level (summary only).
    #[default]
    Info,
}

impl LoggingLayer {
    /// Create a new logging layer with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a logging layer that logs at debug level.
    #[must_use]
    pub fn debug() -> Self {
        Self {
            level: LogLevel::Debug,
        }
    }
}

impl<S> Layer<S> for LoggingLayer {
    type Service = Logging<S>;

    fn layer(&self, inner: S) -> Self::Service {
        Logging {
            inner,
            level: self.level,
        }
    }
}

/// Service that logs requests and responses.
#[derive(Debug, Clone)]
pub struct Logging<S> {
    inner: S,
    level: LogLevel,
}

impl<S> Logging<S> {
    /// Create a new logging service wrapping the given service.
    pub fn new(inner: S) -> Self {
        Self {
            inner,
            level: LogLevel::Info,
        }
    }
}

impl<S> Service<Request<Bytes>> for Logging<S>
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
        let method = request.method();
        let url = request.url().to_string();
        let level = self.level;

        let span = span!(Level::INFO, "http_request", %method, %url);

        let mut inner = self.inner.clone();
        Box::pin(
            async move {
                let start = Instant::now();

                match level {
                    LogLevel::Debug => {
                        debug!(
                            method = %method,
                            url = %url,
                            headers = ?request.headers(),
                            "sending request"
                        );
                    }
                    LogLevel::Info => {
                        info!(method = %method, url = %url, "sending request");
                    }
                }

                let result = inner.call(request).await;
                let elapsed = start.elapsed();

                // Saturating conversion to u64 (truncates after ~584 million years)
                let elapsed_ms = u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX);

                match &result {
                    Ok(response) => {
                        let status = response.status();
                        if response.is_success() {
                            info!(status, elapsed_ms, "request completed");
                        } else {
                            warn!(status, elapsed_ms, "request failed with HTTP error");
                        }
                    }
                    Err(err) => {
                        warn!(error = %err, elapsed_ms, "request failed");
                    }
                }

                result
            }
            .instrument(span),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logging_layer_default() {
        let layer = LoggingLayer::new();
        assert!(matches!(layer.level, LogLevel::Info));
    }

    #[test]
    fn logging_layer_debug() {
        let layer = LoggingLayer::debug();
        assert!(matches!(layer.level, LogLevel::Debug));
    }
}
