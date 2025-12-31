//! HTTP request building.
//!
//! Use [`Request::builder`] to construct requests with headers, query parameters, and bodies.
//!
//! # Example
//!
//! ```
//! use pincer_core::{Request, Method};
//! use bytes::Bytes;
//!
//! let request = Request::<Bytes>::builder(Method::Get, "https://api.example.com".parse().unwrap())
//!     .header("Accept", "application/json")
//!     .query("page", "1")
//!     .build();
//! ```

use std::collections::HashMap;

use bytes::Bytes;
use http::Extensions;

use crate::Method;

/// An HTTP request with method, URL, headers, optional body, and extensions.
#[derive(Debug, Clone)]
pub struct Request<B = Bytes> {
    method: Method,
    url: url::Url,
    headers: HashMap<String, String>,
    body: Option<B>,
    extensions: Extensions,
}

impl<B> Request<B> {
    /// Creates a new [`RequestBuilder`].
    #[must_use]
    pub fn builder(method: Method, url: url::Url) -> RequestBuilder<B> {
        RequestBuilder::new(method, url)
    }

    /// HTTP method.
    #[must_use]
    pub const fn method(&self) -> Method {
        self.method
    }

    /// Request URL.
    #[must_use]
    pub fn url(&self) -> &url::Url {
        &self.url
    }

    /// Request headers.
    #[must_use]
    pub fn headers(&self) -> &HashMap<String, String> {
        &self.headers
    }

    /// Mutable access to headers.
    #[must_use]
    pub fn headers_mut(&mut self) -> &mut HashMap<String, String> {
        &mut self.headers
    }

    /// Single header value by name.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).map(String::as_str)
    }

    /// Request body.
    #[must_use]
    pub const fn body(&self) -> Option<&B> {
        self.body.as_ref()
    }

    /// Request extensions.
    ///
    /// Extensions allow middleware to attach arbitrary typed data to requests.
    #[must_use]
    pub fn extensions(&self) -> &Extensions {
        &self.extensions
    }

    /// Mutable access to extensions.
    #[must_use]
    pub fn extensions_mut(&mut self) -> &mut Extensions {
        &mut self.extensions
    }

    /// Consume into (method, url, headers, body, extensions).
    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        Method,
        url::Url,
        HashMap<String, String>,
        Option<B>,
        Extensions,
    ) {
        (
            self.method,
            self.url,
            self.headers,
            self.body,
            self.extensions,
        )
    }

    /// Construct a request from its parts.
    ///
    /// This is the inverse of [`into_parts`](Self::into_parts) and is useful
    /// for middleware that needs to modify a request and reconstruct it.
    #[must_use]
    pub fn from_parts(
        method: Method,
        url: url::Url,
        headers: HashMap<String, String>,
        body: Option<B>,
        extensions: Extensions,
    ) -> Self {
        Self {
            method,
            url,
            headers,
            body,
            extensions,
        }
    }
}

/// Builder for constructing [`Request`] instances.
#[derive(Debug, Clone)]
pub struct RequestBuilder<B = Bytes> {
    method: Method,
    url: url::Url,
    headers: HashMap<String, String>,
    body: Option<B>,
    extensions: Extensions,
}

impl<B> RequestBuilder<B> {
    /// Creates a new builder.
    #[must_use]
    pub fn new(method: Method, url: url::Url) -> Self {
        Self {
            method,
            url,
            headers: HashMap::new(),
            body: None,
            extensions: Extensions::new(),
        }
    }

    /// Sets a header.
    #[must_use]
    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(name.into(), value.into());
        self
    }

    /// Sets multiple headers.
    #[must_use]
    pub fn headers(mut self, headers: impl IntoIterator<Item = (String, String)>) -> Self {
        self.headers.extend(headers);
        self
    }

    /// Appends a query parameter to the URL.
    #[must_use]
    pub fn query(mut self, name: &str, value: &str) -> Self {
        self.url.query_pairs_mut().append_pair(name, value);
        self
    }

    /// Appends multiple query parameters to the URL.
    #[must_use]
    pub fn query_pairs(mut self, pairs: impl IntoIterator<Item = (String, String)>) -> Self {
        {
            let mut query = self.url.query_pairs_mut();
            for (name, value) in pairs {
                query.append_pair(&name, &value);
            }
        }
        self
    }

    /// Sets the request body.
    #[must_use]
    pub fn body(mut self, body: B) -> Self {
        self.body = Some(body);
        self
    }

    /// Insert a typed extension value.
    ///
    /// Extensions allow middleware to attach arbitrary typed data to requests.
    #[must_use]
    pub fn extension<T: Clone + Send + Sync + 'static>(mut self, value: T) -> Self {
        self.extensions.insert(value);
        self
    }

    /// Set extensions from an existing `Extensions` container.
    ///
    /// This replaces any previously set extensions.
    #[must_use]
    pub fn extensions(mut self, extensions: Extensions) -> Self {
        self.extensions = extensions;
        self
    }

    /// Builds the [`Request`].
    #[must_use]
    pub fn build(self) -> Request<B> {
        Request {
            method: self.method,
            url: self.url,
            headers: self.headers,
            body: self.body,
            extensions: self.extensions,
        }
    }
}

impl RequestBuilder<Bytes> {
    /// Set a JSON body.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization fails.
    pub fn json<T: serde::Serialize>(self, value: &T) -> crate::Result<Self> {
        let body = crate::to_json(value)?;
        Ok(self.header("Content-Type", "application/json").body(body))
    }

    /// Set a form-urlencoded body.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization fails.
    pub fn form<T: serde::Serialize>(self, value: &T) -> crate::Result<Self> {
        let body = crate::to_form(value)?;
        Ok(self
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_builder_basic() {
        let url = url::Url::parse("https://api.example.com/users").expect("valid URL");
        let request = Request::<Bytes>::builder(Method::Get, url.clone())
            .header("Accept", "application/json")
            .build();

        assert_eq!(request.method(), Method::Get);
        assert_eq!(request.url().as_str(), "https://api.example.com/users");
        assert_eq!(request.header("Accept"), Some("application/json"));
        assert!(request.body().is_none());
    }

    #[test]
    fn request_builder_with_query() {
        let url = url::Url::parse("https://api.example.com/users").expect("valid URL");
        let request = Request::<Bytes>::builder(Method::Get, url)
            .query("page", "1")
            .query("limit", "10")
            .build();

        assert_eq!(
            request.url().as_str(),
            "https://api.example.com/users?page=1&limit=10"
        );
    }

    #[test]
    fn request_builder_with_body() {
        let url = url::Url::parse("https://api.example.com/users").expect("valid URL");
        let body = Bytes::from(r#"{"name":"test"}"#);
        let request = Request::builder(Method::Post, url)
            .header("Content-Type", "application/json")
            .body(body.clone())
            .build();

        assert_eq!(request.method(), Method::Post);
        assert_eq!(request.body(), Some(&body));
    }

    #[test]
    fn request_builder_json() {
        #[derive(serde::Serialize)]
        struct User {
            name: String,
        }

        let url = url::Url::parse("https://api.example.com/users").expect("valid URL");
        let request = Request::builder(Method::Post, url)
            .json(&User {
                name: "test".to_string(),
            })
            .expect("json")
            .build();

        assert_eq!(request.header("Content-Type"), Some("application/json"));
        assert!(request.body().is_some());
    }

    #[test]
    fn request_extensions() {
        #[derive(Debug, Clone, PartialEq)]
        struct RequestId(u64);

        let url = url::Url::parse("https://api.example.com").expect("valid URL");
        let mut request = Request::<Bytes>::builder(Method::Get, url)
            .extension(RequestId(42))
            .build();

        // Read extension
        assert_eq!(
            request.extensions().get::<RequestId>(),
            Some(&RequestId(42))
        );

        // Mutate extension
        request.extensions_mut().insert(RequestId(100));
        assert_eq!(
            request.extensions().get::<RequestId>(),
            Some(&RequestId(100))
        );
    }

    #[test]
    fn request_from_parts_roundtrip() {
        #[derive(Debug, Clone, PartialEq)]
        struct TraceId(String);

        let url = url::Url::parse("https://api.example.com").expect("valid URL");
        let request = Request::<Bytes>::builder(Method::Post, url)
            .header("Content-Type", "application/json")
            .body(Bytes::from(r#"{"name":"test"}"#))
            .extension(TraceId("abc123".into()))
            .build();

        let (method, url, headers, body, extensions) = request.into_parts();
        let reconstructed = Request::from_parts(method, url, headers, body, extensions);

        assert_eq!(reconstructed.method(), Method::Post);
        assert_eq!(
            reconstructed.header("Content-Type"),
            Some("application/json")
        );
        assert!(reconstructed.body().is_some());
        assert_eq!(
            reconstructed.extensions().get::<TraceId>(),
            Some(&TraceId("abc123".into()))
        );
    }

    #[test]
    fn request_builder_extensions_replace() {
        #[derive(Debug, Clone, PartialEq)]
        struct Marker(u32);

        let url = url::Url::parse("https://api.example.com").expect("valid URL");

        let mut ext = Extensions::new();
        ext.insert(Marker(99));

        let request = Request::<Bytes>::builder(Method::Get, url)
            .extension(Marker(1)) // This will be replaced
            .extensions(ext)
            .build();

        assert_eq!(request.extensions().get::<Marker>(), Some(&Marker(99)));
    }
}
