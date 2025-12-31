//! HTTP response handling.
//!
//! [`Response`] provides access to status, headers, and body with JSON/text deserialization.
//!
//! # Example
//!
//! ```ignore
//! let user: User = response.json()?;
//! ```
//!
//! For large responses, enable the `streaming` feature for [`streaming::StreamingResponse`].

use std::collections::HashMap;

use bytes::Bytes;

// ============================================================================
// Streaming Response (feature-gated)
// ============================================================================

/// Streaming response support (requires `streaming` feature).
#[cfg(feature = "streaming")]
pub mod streaming {
    use std::collections::HashMap;
    use std::pin::Pin;

    use bytes::Bytes;
    use futures_core::Stream;
    use futures_util::StreamExt;

    /// A streaming body: chunks of bytes arriving over time.
    pub type StreamingBody = Pin<Box<dyn Stream<Item = crate::Result<Bytes>> + Send>>;

    /// HTTP response with streaming body, for large payloads.
    ///
    /// Unlike [`super::Response`], the body is consumed as a stream of chunks.
    pub struct StreamingResponse {
        status: u16,
        headers: HashMap<String, String>,
        body: StreamingBody,
    }

    impl StreamingResponse {
        /// Creates a new streaming response.
        #[must_use]
        pub fn new(status: u16, headers: HashMap<String, String>, body: StreamingBody) -> Self {
            Self {
                status,
                headers,
                body,
            }
        }

        /// HTTP status code.
        #[must_use]
        pub const fn status(&self) -> u16 {
            self.status
        }

        /// Response headers.
        #[must_use]
        pub fn headers(&self) -> &HashMap<String, String> {
            &self.headers
        }

        /// Single header value by name.
        #[must_use]
        pub fn header(&self, name: &str) -> Option<&str> {
            self.headers.get(name).map(String::as_str)
        }

        /// Status is 2xx.
        #[must_use]
        pub const fn is_success(&self) -> bool {
            self.status >= 200 && self.status < 300
        }

        /// Status is 4xx.
        #[must_use]
        pub const fn is_client_error(&self) -> bool {
            self.status >= 400 && self.status < 500
        }

        /// Status is 5xx.
        #[must_use]
        pub const fn is_server_error(&self) -> bool {
            self.status >= 500 && self.status < 600
        }

        /// Consume into the streaming body.
        #[must_use]
        pub fn into_body(self) -> StreamingBody {
            self.body
        }

        /// Buffer the entire stream into a [`Response`].
        ///
        /// # Errors
        ///
        /// Returns an error if reading any chunk fails.
        pub async fn collect(self) -> crate::Result<super::Response<Bytes>> {
            let mut body = self.body;
            let mut collected = Vec::new();

            while let Some(chunk) = body.next().await {
                collected.extend_from_slice(&chunk?);
            }

            Ok(super::Response::new(
                self.status,
                self.headers,
                Bytes::from(collected),
            ))
        }
    }
}

// ============================================================================
// Buffered Response
// ============================================================================

/// HTTP response with status, headers, and body.
#[derive(Debug, Clone)]
pub struct Response<B = Bytes> {
    status: u16,
    headers: HashMap<String, String>,
    body: B,
}

impl<B> Response<B> {
    /// Creates a new response.
    #[must_use]
    pub fn new(status: u16, headers: HashMap<String, String>, body: B) -> Self {
        Self {
            status,
            headers,
            body,
        }
    }

    /// HTTP status code.
    #[must_use]
    pub const fn status(&self) -> u16 {
        self.status
    }

    /// Response headers.
    #[must_use]
    pub fn headers(&self) -> &HashMap<String, String> {
        &self.headers
    }

    /// Single header value by name.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).map(String::as_str)
    }

    /// Response body.
    #[must_use]
    pub const fn body(&self) -> &B {
        &self.body
    }

    /// Consume into body.
    #[must_use]
    pub fn into_body(self) -> B {
        self.body
    }

    /// Consume into (status, headers, body).
    #[must_use]
    pub fn into_parts(self) -> (u16, HashMap<String, String>, B) {
        (self.status, self.headers, self.body)
    }

    /// Status is 2xx.
    #[must_use]
    pub const fn is_success(&self) -> bool {
        self.status >= 200 && self.status < 300
    }

    /// Status is 3xx.
    #[must_use]
    pub const fn is_redirection(&self) -> bool {
        self.status >= 300 && self.status < 400
    }

    /// Status is 4xx.
    #[must_use]
    pub const fn is_client_error(&self) -> bool {
        self.status >= 400 && self.status < 500
    }

    /// Status is 5xx.
    #[must_use]
    pub const fn is_server_error(&self) -> bool {
        self.status >= 500 && self.status < 600
    }

    /// Transform the body with a function.
    pub fn map_body<F, B2>(self, f: F) -> Response<B2>
    where
        F: FnOnce(B) -> B2,
    {
        Response {
            status: self.status,
            headers: self.headers,
            body: f(self.body),
        }
    }
}

impl Response<Bytes> {
    /// Deserialize the response body as JSON.
    ///
    /// # Errors
    ///
    /// Returns an error if deserialization fails.
    pub fn json<T: serde::de::DeserializeOwned>(self) -> crate::Result<T> {
        crate::from_json(&self.body)
    }

    /// Get the response body as text.
    ///
    /// # Errors
    ///
    /// Returns an error if the body is not valid UTF-8.
    pub fn text(self) -> Result<String, std::string::FromUtf8Error> {
        String::from_utf8(self.body.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn response_basic() {
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());

        let response = Response::new(200, headers, Bytes::from(r#"{"id":1}"#));

        assert_eq!(response.status(), 200);
        assert_eq!(response.header("Content-Type"), Some("application/json"));
        assert!(response.is_success());
        assert!(!response.is_client_error());
        assert!(!response.is_server_error());
    }

    #[test]
    fn response_status_checks() {
        let response = Response::new(301, HashMap::new(), Bytes::new());
        assert!(response.is_redirection());

        let response = Response::new(404, HashMap::new(), Bytes::new());
        assert!(response.is_client_error());

        let response = Response::new(500, HashMap::new(), Bytes::new());
        assert!(response.is_server_error());
    }

    #[test]
    fn response_json() {
        #[derive(Debug, PartialEq, serde::Deserialize)]
        struct User {
            id: u64,
            name: String,
        }

        let body = Bytes::from(r#"{"id":1,"name":"test"}"#);
        let response = Response::new(200, HashMap::new(), body);

        let user: User = response.json().expect("deserialize");
        assert_eq!(
            user,
            User {
                id: 1,
                name: "test".to_string()
            }
        );
    }

    #[test]
    fn response_text() {
        let body = Bytes::from("Hello, World!");
        let response = Response::new(200, HashMap::new(), body);

        let text = response.text().expect("text");
        assert_eq!(text, "Hello, World!");
    }

    #[test]
    fn response_map_body() {
        let response = Response::new(200, HashMap::new(), Bytes::from("test"));
        let mapped = response.map_body(|b| b.len());

        assert_eq!(mapped.status(), 200);
        assert_eq!(*mapped.body(), 4);
    }
}
