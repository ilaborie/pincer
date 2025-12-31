//! Generic API client wrapper.
//!
//! This module provides [`ApiClient`], a wrapper that combines any [`HttpClient`]
//! with a base URL to create a [`PincerClient`].

use std::future::Future;

use bytes::Bytes;
use url::Url;

use crate::{Error, HttpClient, PincerClient, Request, Response, Result};

/// Generic API client wrapper.
///
/// Wraps any [`HttpClient`] with a base URL to create a [`PincerClient`].
/// This is useful for sharing a single HTTP client (with its connection pool
/// and middleware) across multiple API traits.
///
/// # Example
///
/// ```ignore
/// use pincer::{ApiClient, HyperClient};
///
/// // Create a shared HTTP client with middleware
/// let http = HyperClient::builder()
///     .with_retry(3)
///     .with_logging()
///     .build();
///
/// // Wrap it for use with different APIs
/// let github = ApiClient::new(http.clone(), "https://api.github.com")?;
/// let gitlab = ApiClient::new(http, "https://gitlab.com/api/v4")?;
/// ```
#[derive(Debug)]
pub struct ApiClient<C> {
    client: C,
    base_url: Url,
}

impl<C: Clone> Clone for ApiClient<C> {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            base_url: self.base_url.clone(),
        }
    }
}

impl<C> ApiClient<C> {
    /// Create a new API client with the given base URL.
    ///
    /// # Errors
    ///
    /// Returns an error if the URL cannot be parsed.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let client = ApiClient::new(http, "https://api.github.com")?;
    /// ```
    pub fn new(client: C, base_url: impl AsRef<str>) -> Result<Self> {
        Ok(Self {
            client,
            base_url: Url::parse(base_url.as_ref()).map_err(Error::InvalidUrl)?,
        })
    }

    /// Create a new API client with a pre-parsed URL.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let url = Url::parse("https://api.github.com")?;
    /// let client = ApiClient::with_url(http, url);
    /// ```
    #[must_use]
    pub fn with_url(client: C, base_url: Url) -> Self {
        Self { client, base_url }
    }

    /// Get a reference to the inner HTTP client.
    #[must_use]
    pub fn inner(&self) -> &C {
        &self.client
    }

    /// Get a mutable reference to the inner HTTP client.
    #[must_use]
    pub fn inner_mut(&mut self) -> &mut C {
        &mut self.client
    }

    /// Consume the wrapper and return the inner HTTP client.
    #[must_use]
    pub fn into_inner(self) -> C {
        self.client
    }
}

impl<C> PincerClient for ApiClient<C>
where
    C: HttpClient + Clone + Send + Sync,
{
    fn execute(
        &self,
        request: Request<Bytes>,
    ) -> impl Future<Output = Result<Response<Bytes>>> + Send {
        self.client.execute(request)
    }

    fn base_url(&self) -> &Url {
        &self.base_url
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_client_new() {
        // We can't easily test without a real HttpClient, but we can test URL parsing
        let url = "https://api.github.com";
        let parsed = Url::parse(url).expect("valid url");
        assert_eq!(parsed.as_str(), "https://api.github.com/");
    }

    #[test]
    fn api_client_invalid_url() {
        let result = Url::parse("not a url");
        assert!(result.is_err());
    }
}
