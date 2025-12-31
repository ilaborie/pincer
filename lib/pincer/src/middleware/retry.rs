//! Retry middleware for HTTP requests.
//!
//! This module provides a simple retry policy for HTTP requests that can be
//! customized based on response status codes and error types.

use std::future;

use bytes::Bytes;
use tower::retry::Policy;

use crate::{Error, Request, Response};

/// A simple retry policy for HTTP requests.
///
/// By default, retries:
/// - Connection errors
/// - 5xx server errors
/// - 429 Too Many Requests
///
/// # Example
///
/// ```ignore
/// use pincer::middleware::{RetryPolicy, ServiceBuilder};
/// use tower::retry::RetryLayer;
///
/// let service = ServiceBuilder::new()
///     .layer(RetryLayer::new(RetryPolicy::new(3)))
///     .service(client);
/// ```
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    remaining: u32,
}

impl RetryPolicy {
    /// Create a new retry policy with the given maximum number of retries.
    #[must_use]
    pub fn new(max_retries: u32) -> Self {
        Self {
            remaining: max_retries,
        }
    }

    /// Returns `true` if the response should be retried.
    fn should_retry_response(response: &Response<Bytes>) -> bool {
        let status = response.status();
        // Retry on 5xx server errors and 429 Too Many Requests
        status >= 500 || status == 429
    }

    /// Returns `true` if the error should be retried.
    fn should_retry_error(error: &Error) -> bool {
        // Retry on connection and timeout errors
        error.is_connection() || error.is_timeout()
    }
}

impl Policy<Request<Bytes>, Response<Bytes>, Error> for RetryPolicy {
    type Future = future::Ready<()>;

    fn retry(
        &mut self,
        _req: &mut Request<Bytes>,
        result: &mut Result<Response<Bytes>, Error>,
    ) -> Option<Self::Future> {
        if self.remaining == 0 {
            return None;
        }

        let should_retry = match result {
            Ok(response) => Self::should_retry_response(response),
            Err(error) => Self::should_retry_error(error),
        };

        if should_retry {
            self.remaining -= 1;
            Some(future::ready(()))
        } else {
            None
        }
    }

    fn clone_request(&mut self, req: &Request<Bytes>) -> Option<Request<Bytes>> {
        // Clone the request for retry
        Some(req.clone())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn retry_policy_new() {
        let policy = RetryPolicy::new(3);
        assert_eq!(policy.remaining, 3);
    }

    #[test]
    fn should_retry_5xx_response() {
        let response = Response::new(500, HashMap::default(), Bytes::new());
        assert!(RetryPolicy::should_retry_response(&response));

        let response = Response::new(503, HashMap::default(), Bytes::new());
        assert!(RetryPolicy::should_retry_response(&response));
    }

    #[test]
    fn should_retry_429_response() {
        let response = Response::new(429, HashMap::default(), Bytes::new());
        assert!(RetryPolicy::should_retry_response(&response));
    }

    #[test]
    fn should_not_retry_4xx_response() {
        let response = Response::new(400, HashMap::default(), Bytes::new());
        assert!(!RetryPolicy::should_retry_response(&response));

        let response = Response::new(404, HashMap::default(), Bytes::new());
        assert!(!RetryPolicy::should_retry_response(&response));
    }

    #[test]
    fn should_not_retry_2xx_response() {
        let response = Response::new(200, HashMap::default(), Bytes::new());
        assert!(!RetryPolicy::should_retry_response(&response));
    }

    #[test]
    fn should_retry_connection_error() {
        let error = Error::connection("connection refused");
        assert!(RetryPolicy::should_retry_error(&error));
    }

    #[test]
    fn should_retry_timeout_error() {
        let error = Error::Timeout;
        assert!(RetryPolicy::should_retry_error(&error));
    }
}
