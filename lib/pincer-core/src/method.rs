//! HTTP method types.

use derive_more::Display;

/// HTTP request method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Display)]
pub enum Method {
    /// GET method - retrieve a resource.
    #[display("GET")]
    Get,
    /// POST method - create a resource.
    #[display("POST")]
    Post,
    /// PUT method - replace a resource.
    #[display("PUT")]
    Put,
    /// DELETE method - remove a resource.
    #[display("DELETE")]
    Delete,
    /// PATCH method - partially update a resource.
    #[display("PATCH")]
    Patch,
    /// HEAD method - retrieve headers only.
    #[display("HEAD")]
    Head,
    /// OPTIONS method - retrieve allowed methods.
    #[display("OPTIONS")]
    Options,
}

impl Method {
    /// Returns `true` if the method is safe (does not modify resources).
    #[must_use]
    pub const fn is_safe(&self) -> bool {
        matches!(self, Self::Get | Self::Head | Self::Options)
    }

    /// Returns `true` if the method is idempotent.
    #[must_use]
    pub const fn is_idempotent(&self) -> bool {
        matches!(
            self,
            Self::Get | Self::Head | Self::Options | Self::Put | Self::Delete
        )
    }
}

impl From<Method> for http::Method {
    fn from(method: Method) -> Self {
        match method {
            Method::Get => Self::GET,
            Method::Post => Self::POST,
            Method::Put => Self::PUT,
            Method::Delete => Self::DELETE,
            Method::Patch => Self::PATCH,
            Method::Head => Self::HEAD,
            Method::Options => Self::OPTIONS,
        }
    }
}

impl TryFrom<http::Method> for Method {
    type Error = crate::Error;

    fn try_from(method: http::Method) -> Result<Self, Self::Error> {
        match method {
            http::Method::GET => Ok(Self::Get),
            http::Method::POST => Ok(Self::Post),
            http::Method::PUT => Ok(Self::Put),
            http::Method::DELETE => Ok(Self::Delete),
            http::Method::PATCH => Ok(Self::Patch),
            http::Method::HEAD => Ok(Self::Head),
            http::Method::OPTIONS => Ok(Self::Options),
            other => Err(crate::Error::InvalidRequest(format!(
                "unsupported HTTP method: {other}"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_display() {
        assert_eq!(Method::Get.to_string(), "GET");
        assert_eq!(Method::Post.to_string(), "POST");
        assert_eq!(Method::Put.to_string(), "PUT");
        assert_eq!(Method::Delete.to_string(), "DELETE");
        assert_eq!(Method::Patch.to_string(), "PATCH");
        assert_eq!(Method::Head.to_string(), "HEAD");
        assert_eq!(Method::Options.to_string(), "OPTIONS");
    }

    #[test]
    fn method_is_safe() {
        assert!(Method::Get.is_safe());
        assert!(Method::Head.is_safe());
        assert!(Method::Options.is_safe());
        assert!(!Method::Post.is_safe());
        assert!(!Method::Put.is_safe());
        assert!(!Method::Delete.is_safe());
        assert!(!Method::Patch.is_safe());
    }

    #[test]
    fn method_is_idempotent() {
        assert!(Method::Get.is_idempotent());
        assert!(Method::Head.is_idempotent());
        assert!(Method::Options.is_idempotent());
        assert!(Method::Put.is_idempotent());
        assert!(Method::Delete.is_idempotent());
        assert!(!Method::Post.is_idempotent());
        assert!(!Method::Patch.is_idempotent());
    }

    #[test]
    fn method_into_http() {
        assert_eq!(http::Method::from(Method::Get), http::Method::GET);
        assert_eq!(http::Method::from(Method::Post), http::Method::POST);
    }

    #[test]
    fn method_from_http() {
        assert_eq!(
            Method::try_from(http::Method::GET).expect("GET"),
            Method::Get
        );
        assert_eq!(
            Method::try_from(http::Method::POST).expect("POST"),
            Method::Post
        );
    }
}
