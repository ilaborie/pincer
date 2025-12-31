//! Follow redirect middleware.
//!
//! This middleware automatically follows HTTP redirects (3xx responses with Location header).
//! It supports configurable maximum redirect count and handles both relative and absolute URLs.

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::Bytes;
use tower::{Layer, Service};
use url::Url;

use crate::{Error, Method, Request, Response, Result};

/// Default maximum number of redirects to follow.
pub const DEFAULT_MAX_REDIRECTS: usize = 10;

/// Layer that follows HTTP redirects.
///
/// # Example
///
/// ```ignore
/// use pincer::middleware::FollowRedirectLayer;
/// use tower::ServiceBuilder;
///
/// let service = ServiceBuilder::new()
///     .layer(FollowRedirectLayer::new())
///     .service(client);
/// ```
#[derive(Debug, Clone)]
pub struct FollowRedirectLayer {
    max_redirects: usize,
}

impl Default for FollowRedirectLayer {
    fn default() -> Self {
        Self::new()
    }
}

impl FollowRedirectLayer {
    /// Create a new follow redirect layer with default max redirects (10).
    #[must_use]
    pub fn new() -> Self {
        Self {
            max_redirects: DEFAULT_MAX_REDIRECTS,
        }
    }

    /// Create a new follow redirect layer with a custom max redirects.
    #[must_use]
    pub fn with_max_redirects(max_redirects: usize) -> Self {
        Self { max_redirects }
    }
}

impl<S> Layer<S> for FollowRedirectLayer {
    type Service = FollowRedirect<S>;

    fn layer(&self, inner: S) -> Self::Service {
        FollowRedirect {
            inner,
            max_redirects: self.max_redirects,
        }
    }
}

/// Service that follows HTTP redirects.
#[derive(Debug, Clone)]
pub struct FollowRedirect<S> {
    inner: S,
    max_redirects: usize,
}

impl<S> FollowRedirect<S> {
    /// Create a new follow redirect service wrapping the given service.
    #[must_use]
    pub fn new(inner: S) -> Self {
        Self {
            inner,
            max_redirects: DEFAULT_MAX_REDIRECTS,
        }
    }

    /// Create a new follow redirect service with a custom max redirects.
    #[must_use]
    pub fn with_max_redirects(inner: S, max_redirects: usize) -> Self {
        Self {
            inner,
            max_redirects,
        }
    }
}

/// Check if a status code is a redirect.
fn is_redirect(status: u16) -> bool {
    matches!(status, 301 | 302 | 303 | 307 | 308)
}

/// Determine the method for the redirected request.
///
/// - 301, 302, 303: Always use GET (standard browser behavior)
/// - 307, 308: Preserve original method
fn redirect_method(status: u16, original: Method) -> Method {
    match status {
        307 | 308 => original,
        _ => Method::Get,
    }
}

/// Resolve a redirect Location URL relative to the original request URL.
fn resolve_redirect_url(base_url: &Url, location: &str) -> Result<Url> {
    // Try parsing as absolute URL first
    if let Ok(url) = Url::parse(location) {
        return Ok(url);
    }

    // Parse as relative URL
    base_url.join(location).map_err(Error::InvalidUrl)
}

impl<S> Service<Request<Bytes>> for FollowRedirect<S>
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
        let mut inner = self.inner.clone();
        let max_redirects = self.max_redirects;

        Box::pin(async move {
            let mut current_request = request;
            let mut redirects = 0;

            loop {
                let response = inner.call(current_request.clone()).await?;

                // Check if this is a redirect
                if !is_redirect(response.status()) {
                    return Ok(response);
                }

                // Check redirect limit
                if redirects >= max_redirects {
                    return Err(Error::TooManyRedirects {
                        count: redirects,
                        max: max_redirects,
                    });
                }

                // Extract Location header
                let location = response
                    .headers()
                    .get("location")
                    .or_else(|| response.headers().get("Location"))
                    .ok_or_else(|| {
                        Error::InvalidRedirect("redirect response missing Location header".into())
                    })?;

                // Resolve the redirect URL
                let current_url = current_request.url();
                let new_url = resolve_redirect_url(current_url, location)?;

                // Determine the new method
                let new_method = redirect_method(response.status(), current_request.method());

                // Build the new request
                // For 303 redirects (and 301/302 to GET), we don't forward the body
                let (_, _, headers, body) = current_request.into_parts();
                let body = if matches!(new_method, Method::Get | Method::Head) {
                    Bytes::new()
                } else {
                    body.unwrap_or_default()
                };

                current_request = Request::builder(new_method, new_url)
                    .headers(headers)
                    .body(body)
                    .build();

                redirects += 1;
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_max_redirects() {
        let layer = FollowRedirectLayer::new();
        assert_eq!(layer.max_redirects, DEFAULT_MAX_REDIRECTS);
    }

    #[test]
    fn custom_max_redirects() {
        let layer = FollowRedirectLayer::with_max_redirects(5);
        assert_eq!(layer.max_redirects, 5);
    }

    #[test]
    fn is_redirect_true() {
        assert!(is_redirect(301));
        assert!(is_redirect(302));
        assert!(is_redirect(303));
        assert!(is_redirect(307));
        assert!(is_redirect(308));
    }

    #[test]
    fn is_redirect_false() {
        assert!(!is_redirect(200));
        assert!(!is_redirect(404));
        assert!(!is_redirect(500));
        assert!(!is_redirect(300)); // 300 Multiple Choices is not auto-followed
        assert!(!is_redirect(304)); // 304 Not Modified is not a redirect
    }

    #[test]
    fn redirect_method_to_get() {
        // 301, 302, 303 should become GET
        assert!(matches!(redirect_method(301, Method::Post), Method::Get));
        assert!(matches!(redirect_method(302, Method::Put), Method::Get));
        assert!(matches!(redirect_method(303, Method::Delete), Method::Get));
    }

    #[test]
    fn redirect_method_preserved() {
        // 307, 308 should preserve method
        assert!(matches!(redirect_method(307, Method::Post), Method::Post));
        assert!(matches!(redirect_method(308, Method::Put), Method::Put));
    }

    #[test]
    fn resolve_absolute_url() {
        let base = Url::parse("https://example.com/path").expect("base url");
        let result = resolve_redirect_url(&base, "https://other.com/new").expect("resolve");
        assert_eq!(result.as_str(), "https://other.com/new");
    }

    #[test]
    fn resolve_relative_url() {
        let base = Url::parse("https://example.com/old/path").expect("base url");
        let result = resolve_redirect_url(&base, "/new/path").expect("resolve");
        assert_eq!(result.as_str(), "https://example.com/new/path");
    }

    #[test]
    fn resolve_relative_url_without_leading_slash() {
        let base = Url::parse("https://example.com/old/path").expect("base url");
        let result = resolve_redirect_url(&base, "sibling").expect("resolve");
        assert_eq!(result.as_str(), "https://example.com/old/sibling");
    }
}
