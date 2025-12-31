//! Parameter metadata for runtime access.
//!
//! This module provides types that allow middleware to inspect parameter
//! information at runtime, useful for `OpenAPI` generation, validation,
//! and debugging.

use std::fmt;

/// Parameter location in the HTTP request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParamLocation {
    /// Path parameter (e.g., `/users/{id}`)
    Path,
    /// Query parameter (e.g., `?limit=10`)
    Query,
    /// Header parameter
    Header,
    /// Request body (JSON)
    Body,
    /// Form data (URL-encoded or multipart)
    Form,
}

impl fmt::Display for ParamLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Path => write!(f, "path"),
            Self::Query => write!(f, "query"),
            Self::Header => write!(f, "header"),
            Self::Body => write!(f, "body"),
            Self::Form => write!(f, "form"),
        }
    }
}

/// Metadata about a single parameter.
///
/// This struct contains information about a method parameter that
/// can be used for documentation, validation, or debugging.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParamMeta {
    /// The parameter name as declared in the method signature.
    pub name: &'static str,
    /// Where the parameter is sent in the HTTP request.
    pub location: ParamLocation,
    /// The Rust type name (e.g., "u64", "Option<String>").
    pub type_name: &'static str,
    /// Whether the parameter is required (not an Option type).
    pub required: bool,
}

/// All parameter metadata for a method call.
///
/// This is stored in request extensions to allow middleware to access
/// parameter information at runtime.
///
/// # Example
///
/// ```ignore
/// // In middleware
/// if let Some(meta) = request.extensions().get::<ParameterMetadata>() {
///     println!("Method: {}", meta.method_name);
///     for param in meta.parameters {
///         println!("  {}: {} ({:?})", param.name, param.type_name, param.location);
///     }
/// }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ParameterMetadata {
    /// The method name that was called.
    pub method_name: &'static str,
    /// Metadata for each parameter.
    pub parameters: &'static [ParamMeta],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn param_location_display() {
        assert_eq!(ParamLocation::Path.to_string(), "path");
        assert_eq!(ParamLocation::Query.to_string(), "query");
        assert_eq!(ParamLocation::Header.to_string(), "header");
        assert_eq!(ParamLocation::Body.to_string(), "body");
        assert_eq!(ParamLocation::Form.to_string(), "form");
    }

    #[test]
    fn param_meta_construction() {
        let meta = ParamMeta {
            name: "id",
            location: ParamLocation::Path,
            type_name: "u64",
            required: true,
        };
        assert_eq!(meta.name, "id");
        assert_eq!(meta.location, ParamLocation::Path);
        assert_eq!(meta.type_name, "u64");
        assert!(meta.required);
    }

    #[test]
    fn parameter_metadata_construction() {
        static PARAMS: &[ParamMeta] = &[
            ParamMeta {
                name: "id",
                location: ParamLocation::Path,
                type_name: "u64",
                required: true,
            },
            ParamMeta {
                name: "limit",
                location: ParamLocation::Query,
                type_name: "Option<u32>",
                required: false,
            },
        ];

        let meta = ParameterMetadata {
            method_name: "get_user",
            parameters: PARAMS,
        };

        assert_eq!(meta.method_name, "get_user");
        assert_eq!(meta.parameters.len(), 2);
        assert_eq!(meta.parameters.first().expect("first param").name, "id");
        assert_eq!(meta.parameters.get(1).expect("second param").name, "limit");
    }

    #[test]
    fn parameter_metadata_default() {
        let meta = ParameterMetadata::default();
        assert_eq!(meta.method_name, "");
        assert!(meta.parameters.is_empty());
    }
}
