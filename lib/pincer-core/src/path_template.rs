//! Path template for middleware access.

/// The original path template before parameter substitution.
///
/// This is stored in request extensions to allow middleware to access
/// the template pattern (e.g., `/users/{id}`) rather than the resolved
/// path (e.g., `/users/123`).
///
/// # Example
///
/// ```ignore
/// // In middleware
/// if let Some(template) = request.extensions().get::<PathTemplate>() {
///     println!("Path template: {}", template.as_str());
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PathTemplate(&'static str);

impl PathTemplate {
    /// Create a new path template.
    #[must_use]
    pub const fn new(template: &'static str) -> Self {
        Self(template)
    }

    /// Get the template string.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        self.0
    }
}

impl std::fmt::Display for PathTemplate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for PathTemplate {
    fn as_ref(&self) -> &str {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_template_as_str() {
        let template = PathTemplate::new("/users/{id}/posts/{post_id}");
        assert_eq!(template.as_str(), "/users/{id}/posts/{post_id}");
    }

    #[test]
    fn path_template_as_ref() {
        let template = PathTemplate::new("/users/{id}");
        let s: &str = template.as_ref();
        assert_eq!(s, "/users/{id}");
    }
}
