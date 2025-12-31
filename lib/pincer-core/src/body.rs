//! Body serialization utilities.

use bytes::Bytes;

use crate::Result;

/// Content type for request bodies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContentType {
    /// JSON content type (`application/json`).
    Json,
    /// Form URL-encoded content type (`application/x-www-form-urlencoded`).
    FormUrlEncoded,
    /// Plain text content type (`text/plain`).
    PlainText,
    /// Binary content type (`application/octet-stream`).
    OctetStream,
}

impl ContentType {
    /// Get the MIME type string.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Json => "application/json",
            Self::FormUrlEncoded => "application/x-www-form-urlencoded",
            Self::PlainText => "text/plain",
            Self::OctetStream => "application/octet-stream",
        }
    }
}

impl std::fmt::Display for ContentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Serialize a value to JSON bytes.
///
/// # Errors
///
/// Returns an error if JSON serialization fails.
///
/// # Example
///
/// ```
/// use pincer_core::to_json;
/// use serde::Serialize;
///
/// #[derive(Serialize)]
/// struct User { name: String }
///
/// let user = User { name: "Alice".to_string() };
/// let bytes = to_json(&user).expect("serialize");
/// assert_eq!(bytes.as_ref(), br#"{"name":"Alice"}"#);
/// ```
pub fn to_json<T: serde::Serialize>(value: &T) -> Result<Bytes> {
    serde_json::to_vec(value)
        .map(Bytes::from)
        .map_err(Into::into)
}

/// Serialize a value to form URL-encoded bytes.
///
/// Uses `serde_html_form` which supports `Vec<T>` for repeated form fields
/// (e.g., `tags=a&tags=b&tags=c`).
///
/// # Errors
///
/// Returns an error if form serialization fails.
///
/// # Example
///
/// ```
/// use pincer_core::to_form;
/// use serde::Serialize;
///
/// #[derive(Serialize)]
/// struct Login { username: String, password: String }
///
/// let login = Login { username: "alice".to_string(), password: "secret".to_string() };
/// let bytes = to_form(&login).expect("serialize");
/// assert_eq!(bytes.as_ref(), b"username=alice&password=secret");
/// ```
pub fn to_form<T: serde::Serialize>(value: &T) -> Result<Bytes> {
    serde_html_form::to_string(value)
        .map(|s| Bytes::from(s.into_bytes()))
        .map_err(Into::into)
}

/// Serialize a value to a query string.
///
/// Uses `serde_html_form` which supports `Vec<T>` for repeated query parameters
/// (e.g., `?tags=a&tags=b&tags=c`).
///
/// # Errors
///
/// Returns an error if query serialization fails.
///
/// # Example
///
/// ```
/// use pincer_core::to_query_string;
/// use serde::Serialize;
///
/// #[derive(Serialize)]
/// struct Search {
///     q: String,
///     #[serde(skip_serializing_if = "Option::is_none")]
///     page: Option<u32>,
/// }
///
/// let search = Search { q: "rust".to_string(), page: Some(1) };
/// let query = to_query_string(&search).expect("serialize");
/// assert_eq!(query, "q=rust&page=1");
/// ```
pub fn to_query_string<T: serde::Serialize>(value: &T) -> Result<String> {
    serde_html_form::to_string(value).map_err(Into::into)
}

/// Deserialize JSON bytes to a value with path-aware error messages.
///
/// Uses `serde_path_to_error` to provide detailed error messages that include
/// the exact path to the field that failed to deserialize.
///
/// # Errors
///
/// Returns an error if JSON deserialization fails, with the error message
/// including the path to the problematic field (e.g., "user.address.city").
///
/// # Example
///
/// ```
/// use pincer_core::from_json;
/// use serde::Deserialize;
///
/// #[derive(Debug, PartialEq, Deserialize)]
/// struct User { name: String }
///
/// let bytes = br#"{"name":"Alice"}"#;
/// let user: User = from_json(bytes).expect("deserialize");
/// assert_eq!(user, User { name: "Alice".to_string() });
/// ```
pub fn from_json<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    serde_path_to_error::deserialize(&mut deserializer).map_err(|e| {
        crate::Error::json_deserialization(e.path().to_string(), e.inner().to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_type_as_str() {
        assert_eq!(ContentType::Json.as_str(), "application/json");
        assert_eq!(
            ContentType::FormUrlEncoded.as_str(),
            "application/x-www-form-urlencoded"
        );
        assert_eq!(ContentType::PlainText.as_str(), "text/plain");
        assert_eq!(
            ContentType::OctetStream.as_str(),
            "application/octet-stream"
        );
    }

    #[test]
    fn content_type_display() {
        assert_eq!(ContentType::Json.to_string(), "application/json");
    }

    #[test]
    fn to_json_serialize() {
        #[derive(serde::Serialize)]
        struct User {
            name: String,
            age: u32,
        }

        let user = User {
            name: "Alice".to_string(),
            age: 30,
        };

        let bytes = to_json(&user).expect("serialize");
        assert_eq!(bytes.as_ref(), br#"{"name":"Alice","age":30}"#);
    }

    #[test]
    fn to_form_serialize() {
        #[derive(serde::Serialize)]
        struct Login {
            username: String,
            password: String,
        }

        let login = Login {
            username: "alice".to_string(),
            password: "secret".to_string(),
        };

        let bytes = to_form(&login).expect("serialize");
        assert_eq!(bytes.as_ref(), b"username=alice&password=secret");
    }

    #[test]
    fn to_form_with_vec() {
        #[derive(serde::Serialize)]
        struct TaggedItem {
            name: String,
            tags: Vec<String>,
        }

        let item = TaggedItem {
            name: "test".to_string(),
            tags: vec!["rust".to_string(), "http".to_string(), "async".to_string()],
        };

        let bytes = to_form(&item).expect("serialize");
        let result = String::from_utf8(bytes.to_vec()).expect("utf8");
        // serde_html_form supports repeated params for Vec<T>
        assert!(result.contains("name=test"));
        assert!(result.contains("tags=rust"));
        assert!(result.contains("tags=http"));
        assert!(result.contains("tags=async"));
    }

    #[test]
    fn to_query_string_with_option() {
        #[derive(serde::Serialize)]
        struct Search {
            q: String,
            #[serde(skip_serializing_if = "Option::is_none")]
            page: Option<u32>,
        }

        let search = Search {
            q: "rust".to_string(),
            page: Some(1),
        };
        let query = to_query_string(&search).expect("serialize");
        assert_eq!(query, "q=rust&page=1");

        let search_no_page = Search {
            q: "rust".to_string(),
            page: None,
        };
        let query = to_query_string(&search_no_page).expect("serialize");
        assert_eq!(query, "q=rust");
    }

    #[test]
    fn to_query_string_with_vec() {
        #[derive(serde::Serialize)]
        struct Filter {
            tags: Vec<String>,
        }

        let filter = Filter {
            tags: vec!["a".to_string(), "b".to_string(), "c".to_string()],
        };

        let query = to_query_string(&filter).expect("serialize");
        // serde_html_form produces repeated params: tags=a&tags=b&tags=c
        assert!(query.contains("tags=a"));
        assert!(query.contains("tags=b"));
        assert!(query.contains("tags=c"));
    }

    #[test]
    fn from_json_deserialize() {
        #[derive(Debug, PartialEq, serde::Deserialize)]
        struct User {
            name: String,
            age: u32,
        }

        let bytes = br#"{"name":"Alice","age":30}"#;
        let user: User = from_json(bytes).expect("deserialize");

        assert_eq!(
            user,
            User {
                name: "Alice".to_string(),
                age: 30,
            }
        );
    }

    #[test]
    fn from_json_syntax_error() {
        #[derive(Debug, serde::Deserialize)]
        struct User {
            #[allow(dead_code)]
            name: String,
        }

        let bytes = b"not json";
        let result: Result<User> = from_json(bytes);

        assert!(result.is_err());
        let err = result.expect_err("should fail");
        // Syntax errors have empty path
        assert!(err.to_string().contains("JSON deserialization error"));
    }

    #[test]
    fn from_json_missing_field_error_with_path() {
        #[derive(Debug, serde::Deserialize)]
        struct Address {
            #[allow(dead_code)]
            city: String,
        }

        #[derive(Debug, serde::Deserialize)]
        struct User {
            #[allow(dead_code)]
            address: Address,
        }

        // Missing 'city' field inside 'address'
        let bytes = br#"{"address":{}}"#;
        let result: Result<User> = from_json(bytes);

        assert!(result.is_err());
        let err = result.expect_err("should fail");
        let msg = err.to_string();
        // Should include the path context and mention the missing field
        assert!(
            msg.contains("address"),
            "Expected path 'address' in error: {msg}"
        );
        assert!(
            msg.contains("city"),
            "Expected field 'city' mentioned in error: {msg}"
        );
    }
}
