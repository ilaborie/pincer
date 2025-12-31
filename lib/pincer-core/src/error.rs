//! Error types for pincer.

use derive_more::{Display, Error, From};

// ============================================================================
// Error Decoder Trait
// ============================================================================

/// Trait for decoding HTTP error responses into typed errors.
///
/// Implement this trait to customize how error responses are handled.
/// The decoder receives the HTTP status code and response body, and can
/// optionally return a decoded error.
///
/// # Example
///
/// ```ignore
/// use pincer::ErrorDecoder;
///
/// #[derive(Debug, Deserialize)]
/// struct ApiError {
///     code: String,
///     message: String,
/// }
///
/// struct MyErrorDecoder;
///
/// impl ErrorDecoder for MyErrorDecoder {
///     type Error = ApiError;
///
///     fn decode(&self, status: u16, body: &bytes::Bytes) -> Option<Self::Error> {
///         if status >= 400 {
///             serde_json::from_slice(body).ok()
///         } else {
///             None
///         }
///     }
/// }
/// ```
pub trait ErrorDecoder: Send + Sync + 'static {
    /// The decoded error type.
    type Error: std::fmt::Debug + Send + Sync + 'static;

    /// Decode an HTTP error response into a typed error.
    ///
    /// Returns `Some(error)` if the response should be decoded as a custom error,
    /// or `None` to fall back to the default `Error::Http` handling.
    fn decode(&self, status: u16, body: &bytes::Bytes) -> Option<Self::Error>;
}

/// Default error decoder that always returns `None`.
///
/// This is the default decoder used when no custom decoder is specified.
/// It simply falls back to the standard `Error::Http` handling.
#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultErrorDecoder;

impl ErrorDecoder for DefaultErrorDecoder {
    type Error = std::convert::Infallible;

    fn decode(&self, _status: u16, _body: &bytes::Bytes) -> Option<Self::Error> {
        None
    }
}

// ============================================================================
// Error Type
// ============================================================================

/// Main error type for pincer operations.
#[derive(Debug, Display, Error, From)]
pub enum Error {
    /// HTTP-level errors (non-2xx status codes).
    #[display("HTTP error {status}: {message}")]
    #[from(skip)]
    Http {
        /// HTTP status code.
        status: u16,
        /// Error message.
        message: String,
        /// Response body, if available.
        #[error(not(source))]
        body: Option<bytes::Bytes>,
    },

    /// Network/connection errors.
    #[display("connection error: {_0}")]
    #[from(skip)]
    Connection(#[error(not(source))] String),

    /// TLS/SSL errors.
    #[display("TLS error: {_0}")]
    #[from(skip)]
    Tls(#[error(not(source))] String),

    /// Request timeout.
    #[display("request timeout")]
    #[from(skip)]
    Timeout,

    /// Invalid request configuration.
    #[display("invalid request: {_0}")]
    #[from(skip)]
    InvalidRequest(#[error(not(source))] String),

    /// JSON serialization error.
    #[display("JSON serialization error: {_0}")]
    #[from]
    JsonSerialization(serde_json::Error),

    /// JSON deserialization error with path context.
    #[display("JSON deserialization error at '{path}': {message}")]
    #[from(skip)]
    JsonDeserialization {
        /// JSON path to the error (e.g., "user.address.city").
        path: String,
        /// Error message.
        message: String,
    },

    /// Form URL-encoded serialization error.
    #[display("form serialization error: {_0}")]
    #[from]
    FormSerialization(serde_urlencoded::ser::Error),

    /// Query string serialization error.
    #[display("query serialization error: {_0}")]
    #[from]
    QuerySerialization(serde_html_form::ser::Error),

    /// URL parsing error.
    #[display("invalid URL: {_0}")]
    #[from]
    InvalidUrl(url::ParseError),

    /// Too many redirects.
    #[display("too many redirects ({count} exceeded max of {max})")]
    #[from(skip)]
    TooManyRedirects {
        /// Number of redirects followed.
        count: usize,
        /// Maximum allowed redirects.
        max: usize,
    },

    /// Invalid redirect response.
    #[display("invalid redirect: {_0}")]
    #[from(skip)]
    InvalidRedirect(#[error(not(source))] String),
}

/// Result type alias using [`crate::Error`].
pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// Create an HTTP error from status code and message.
    #[must_use]
    pub fn http(status: u16, message: impl Into<String>) -> Self {
        Self::Http {
            status,
            message: message.into(),
            body: None,
        }
    }

    /// Create an HTTP error with body.
    #[must_use]
    pub fn http_with_body(status: u16, message: impl Into<String>, body: bytes::Bytes) -> Self {
        Self::Http {
            status,
            message: message.into(),
            body: Some(body),
        }
    }

    /// Create a connection error.
    #[must_use]
    pub fn connection(message: impl Into<String>) -> Self {
        Self::Connection(message.into())
    }

    /// Create a TLS error.
    #[must_use]
    pub fn tls(message: impl Into<String>) -> Self {
        Self::Tls(message.into())
    }

    /// Create an invalid request error.
    #[must_use]
    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self::InvalidRequest(message.into())
    }

    /// Create a JSON deserialization error with path context.
    #[must_use]
    pub fn json_deserialization(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self::JsonDeserialization {
            path: path.into(),
            message: message.into(),
        }
    }

    /// Returns `true` if this is a timeout error.
    #[must_use]
    pub const fn is_timeout(&self) -> bool {
        matches!(self, Self::Timeout)
    }

    /// Returns `true` if this is a connection error.
    #[must_use]
    pub const fn is_connection(&self) -> bool {
        matches!(self, Self::Connection(_))
    }

    /// Returns the HTTP status code if this is an HTTP error.
    #[must_use]
    pub const fn status(&self) -> Option<u16> {
        match self {
            Self::Http { status, .. } => Some(*status),
            _ => None,
        }
    }

    /// Returns `true` if this is a client error (4xx).
    #[must_use]
    pub fn is_client_error(&self) -> bool {
        self.status().is_some_and(|s| (400..500).contains(&s))
    }

    /// Returns `true` if this is a server error (5xx).
    #[must_use]
    pub fn is_server_error(&self) -> bool {
        self.status().is_some_and(|s| (500..600).contains(&s))
    }

    /// Returns `true` if this is a 404 Not Found error.
    #[must_use]
    pub fn is_not_found(&self) -> bool {
        self.status() == Some(404)
    }

    /// Returns the response body if this is an HTTP error with a body.
    #[must_use]
    pub fn body(&self) -> Option<&bytes::Bytes> {
        match self {
            Self::Http { body, .. } => body.as_ref(),
            _ => None,
        }
    }

    /// Try to decode the HTTP error body as JSON.
    ///
    /// Returns `Some(Ok(value))` if the error has a body and it deserializes successfully,
    /// `Some(Err(error))` if the body exists but deserialization fails,
    /// or `None` if there is no body or this is not an HTTP error.
    ///
    /// # Example
    ///
    /// ```ignore
    /// #[derive(Debug, Deserialize)]
    /// struct ApiError {
    ///     code: String,
    ///     message: String,
    /// }
    ///
    /// match client.get_user(123).await {
    ///     Ok(user) => println!("User: {:?}", user),
    ///     Err(e) => {
    ///         if let Some(Ok(api_error)) = e.decode_body::<ApiError>() {
    ///             println!("API error: {} - {}", api_error.code, api_error.message);
    ///         } else {
    ///             println!("Error: {}", e);
    ///         }
    ///     }
    /// }
    /// ```
    pub fn decode_body<T: serde::de::DeserializeOwned>(&self) -> Option<Result<T>> {
        self.body().map(|body| crate::from_json(body))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display() {
        let err = Error::http(404, "Not Found");
        assert_eq!(err.to_string(), "HTTP error 404: Not Found");

        let err = Error::Timeout;
        assert_eq!(err.to_string(), "request timeout");

        let err = Error::connection("failed to connect");
        assert_eq!(err.to_string(), "connection error: failed to connect");

        let err = Error::json_deserialization("user.address.city", "missing field `city`");
        assert_eq!(
            err.to_string(),
            "JSON deserialization error at 'user.address.city': missing field `city`"
        );
    }

    #[test]
    fn error_status() {
        let err = Error::http(404, "Not Found");
        assert_eq!(err.status(), Some(404));
        assert!(err.is_client_error());
        assert!(!err.is_server_error());

        let err = Error::http(500, "Internal Server Error");
        assert_eq!(err.status(), Some(500));
        assert!(!err.is_client_error());
        assert!(err.is_server_error());

        let err = Error::Timeout;
        assert_eq!(err.status(), None);
        assert!(!err.is_client_error());
        assert!(!err.is_server_error());
    }

    #[test]
    fn error_is_timeout() {
        assert!(Error::Timeout.is_timeout());
        assert!(!Error::http(404, "Not Found").is_timeout());
    }

    #[test]
    fn error_is_connection() {
        assert!(Error::connection("failed").is_connection());
        assert!(!Error::Timeout.is_connection());
    }

    #[test]
    fn error_is_not_found() {
        assert!(Error::http(404, "Not Found").is_not_found());
        assert!(!Error::http(400, "Bad Request").is_not_found());
        assert!(!Error::Timeout.is_not_found());
    }

    #[test]
    fn error_body() {
        let err = Error::http(404, "Not Found");
        assert!(err.body().is_none());

        let body = bytes::Bytes::from(r#"{"error": "not found"}"#);
        let err = Error::http_with_body(404, "Not Found", body.clone());
        assert_eq!(err.body(), Some(&body));

        assert!(Error::Timeout.body().is_none());
    }

    #[test]
    fn error_decode_body() {
        #[derive(Debug, PartialEq, serde::Deserialize)]
        struct ApiError {
            error: String,
        }

        let body = bytes::Bytes::from(r#"{"error": "not found"}"#);
        let err = Error::http_with_body(404, "Not Found", body);

        let decoded = err.decode_body::<ApiError>();
        assert!(decoded.is_some());
        let result = decoded.expect("should have body");
        assert!(result.is_ok());
        assert_eq!(
            result.expect("should decode"),
            ApiError {
                error: "not found".to_string()
            }
        );

        // No body
        let err = Error::http(404, "Not Found");
        assert!(err.decode_body::<ApiError>().is_none());

        // Non-HTTP error
        assert!(Error::Timeout.decode_body::<ApiError>().is_none());
    }

    #[test]
    fn default_error_decoder() {
        let decoder = DefaultErrorDecoder;
        let body = bytes::Bytes::from("test");
        assert!(decoder.decode(404, &body).is_none());
        assert!(decoder.decode(500, &body).is_none());
    }
}
