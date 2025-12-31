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

use crate::Method;

/// An HTTP request with method, URL, headers, and optional body.
#[derive(Debug, Clone)]
pub struct Request<B = Bytes> {
    method: Method,
    url: url::Url,
    headers: HashMap<String, String>,
    body: Option<B>,
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

    /// Consume into (method, url, headers, body).
    #[must_use]
    pub fn into_parts(self) -> (Method, url::Url, HashMap<String, String>, Option<B>) {
        (self.method, self.url, self.headers, self.body)
    }
}

/// Builder for constructing [`Request`] instances.
#[derive(Debug, Clone)]
pub struct RequestBuilder<B = Bytes> {
    method: Method,
    url: url::Url,
    headers: HashMap<String, String>,
    body: Option<B>,
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

    /// Builds the [`Request`].
    #[must_use]
    pub fn build(self) -> Request<B> {
        Request {
            method: self.method,
            url: self.url,
            headers: self.headers,
            body: self.body,
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
}
