//! Multipart form data support for file uploads.
//!
//! This module provides types for building multipart/form-data requests,
//! commonly used for file uploads and form submissions with binary data.
//!
//! # Example
//!
//! ```ignore
//! use pincer::multipart::{Form, Part};
//!
//! let form = Form::new()
//!     .part(Part::text("name", "John Doe"))
//!     .part(Part::file("avatar", "photo.jpg", photo_bytes));
//!
//! let (content_type, body) = form.into_body();
//! ```

use bytes::{BufMut, Bytes, BytesMut};

/// A single part in a multipart form.
///
/// Each part can be text, binary data, or a file with optional filename
/// and content type.
#[derive(Debug, Clone)]
pub struct Part {
    name: String,
    filename: Option<String>,
    content_type: Option<String>,
    data: Bytes,
}

impl Part {
    /// Create a new part with the given name and data.
    #[must_use]
    pub fn new(name: impl Into<String>, data: impl Into<Bytes>) -> Self {
        Self {
            name: name.into(),
            filename: None,
            content_type: None,
            data: data.into(),
        }
    }

    /// Create a text part.
    ///
    /// Sets the content type to `text/plain; charset=utf-8`.
    #[must_use]
    pub fn text(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            filename: None,
            content_type: Some("text/plain; charset=utf-8".to_string()),
            data: Bytes::from(value.into()),
        }
    }

    /// Create a binary part.
    ///
    /// Sets the content type to `application/octet-stream`.
    #[must_use]
    pub fn bytes(name: impl Into<String>, data: impl Into<Bytes>) -> Self {
        Self {
            name: name.into(),
            filename: None,
            content_type: Some("application/octet-stream".to_string()),
            data: data.into(),
        }
    }

    /// Create a file part with filename.
    ///
    /// The content type is guessed from the filename extension, or defaults
    /// to `application/octet-stream` if unknown.
    #[must_use]
    pub fn file(
        name: impl Into<String>,
        filename: impl Into<String>,
        data: impl Into<Bytes>,
    ) -> Self {
        let filename = filename.into();
        let content_type = guess_content_type(&filename);
        Self {
            name: name.into(),
            filename: Some(filename),
            content_type: Some(content_type),
            data: data.into(),
        }
    }

    /// Set the filename for this part.
    #[must_use]
    pub fn with_filename(mut self, filename: impl Into<String>) -> Self {
        self.filename = Some(filename.into());
        self
    }

    /// Set the content type for this part.
    #[must_use]
    pub fn with_content_type(mut self, content_type: impl Into<String>) -> Self {
        self.content_type = Some(content_type.into());
        self
    }

    /// Get the part name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the filename, if set.
    #[must_use]
    pub fn filename(&self) -> Option<&str> {
        self.filename.as_deref()
    }

    /// Get the content type, if set.
    #[must_use]
    pub fn content_type(&self) -> Option<&str> {
        self.content_type.as_deref()
    }

    /// Get the part data.
    #[must_use]
    pub fn data(&self) -> &Bytes {
        &self.data
    }
}

/// Guess the content type from a filename extension.
fn guess_content_type(filename: &str) -> String {
    let extension = filename
        .rsplit('.')
        .next()
        .map(str::to_lowercase)
        .unwrap_or_default();

    match extension.as_str() {
        // Images
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        "bmp" => "image/bmp",
        // Documents
        "pdf" => "application/pdf",
        "doc" => "application/msword",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xls" => "application/vnd.ms-excel",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "ppt" => "application/vnd.ms-powerpoint",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        // Text
        "txt" => "text/plain",
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "js" => "application/javascript",
        "json" => "application/json",
        "xml" => "application/xml",
        "csv" => "text/csv",
        "md" => "text/markdown",
        // Archives
        "zip" => "application/zip",
        "tar" => "application/x-tar",
        "gz" | "gzip" => "application/gzip",
        "rar" => "application/vnd.rar",
        "7z" => "application/x-7z-compressed",
        // Audio/Video
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "ogg" => "audio/ogg",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "avi" => "video/x-msvideo",
        // Other
        "wasm" => "application/wasm",
        _ => "application/octet-stream",
    }
    .to_string()
}

/// A multipart form containing multiple parts.
///
/// Use the builder pattern to construct a form with multiple parts,
/// then convert it to a body with `into_body()`.
#[derive(Debug, Clone)]
pub struct Form {
    parts: Vec<Part>,
    boundary: String,
}

impl Default for Form {
    fn default() -> Self {
        Self::new()
    }
}

impl Form {
    /// Create a new empty form with a random boundary.
    #[must_use]
    pub fn new() -> Self {
        Self {
            parts: Vec::new(),
            boundary: generate_boundary(),
        }
    }

    /// Create a new form with a custom boundary.
    ///
    /// The boundary should be a unique string that doesn't appear in any part data.
    #[must_use]
    pub fn with_boundary(boundary: impl Into<String>) -> Self {
        Self {
            parts: Vec::new(),
            boundary: boundary.into(),
        }
    }

    /// Add a part to the form.
    #[must_use]
    pub fn part(mut self, part: Part) -> Self {
        self.parts.push(part);
        self
    }

    /// Add a text field to the form.
    #[must_use]
    pub fn text(self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.part(Part::text(name, value))
    }

    /// Add a file to the form.
    #[must_use]
    pub fn file(
        self,
        name: impl Into<String>,
        filename: impl Into<String>,
        data: impl Into<Bytes>,
    ) -> Self {
        self.part(Part::file(name, filename, data))
    }

    /// Get the boundary string.
    #[must_use]
    pub fn boundary(&self) -> &str {
        &self.boundary
    }

    /// Get the parts in this form.
    #[must_use]
    pub fn parts(&self) -> &[Part] {
        &self.parts
    }

    /// Get the Content-Type header value for this form.
    ///
    /// Returns `multipart/form-data; boundary=<boundary>`.
    #[must_use]
    pub fn content_type(&self) -> String {
        format!("multipart/form-data; boundary={}", self.boundary)
    }

    /// Convert the form into a body.
    ///
    /// Returns a tuple of (content-type header value, body bytes).
    #[must_use]
    pub fn into_body(self) -> (String, Bytes) {
        let content_type = self.content_type();
        let body = self.encode();
        (content_type, body)
    }

    /// Encode the form into bytes.
    fn encode(&self) -> Bytes {
        let mut buf = BytesMut::new();

        for part in &self.parts {
            // Boundary
            buf.put_slice(b"--");
            buf.put_slice(self.boundary.as_bytes());
            buf.put_slice(b"\r\n");

            // Content-Disposition
            buf.put_slice(b"Content-Disposition: form-data; name=\"");
            buf.put_slice(part.name.as_bytes());
            buf.put_slice(b"\"");
            if let Some(filename) = &part.filename {
                buf.put_slice(b"; filename=\"");
                buf.put_slice(filename.as_bytes());
                buf.put_slice(b"\"");
            }
            buf.put_slice(b"\r\n");

            // Content-Type (optional)
            if let Some(content_type) = &part.content_type {
                buf.put_slice(b"Content-Type: ");
                buf.put_slice(content_type.as_bytes());
                buf.put_slice(b"\r\n");
            }

            // Empty line before data
            buf.put_slice(b"\r\n");

            // Data
            buf.put_slice(&part.data);
            buf.put_slice(b"\r\n");
        }

        // Final boundary
        buf.put_slice(b"--");
        buf.put_slice(self.boundary.as_bytes());
        buf.put_slice(b"--\r\n");

        buf.freeze()
    }
}

/// Generate a random boundary string.
fn generate_boundary() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);

    format!("----PincerBoundary{timestamp:x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn part_text() {
        let part = Part::text("field", "value");
        assert_eq!(part.name(), "field");
        assert_eq!(part.data().as_ref(), b"value");
        assert_eq!(part.content_type(), Some("text/plain; charset=utf-8"));
        assert!(part.filename().is_none());
    }

    #[test]
    fn part_bytes() {
        let part = Part::bytes("data", vec![1, 2, 3]);
        assert_eq!(part.name(), "data");
        assert_eq!(part.data().as_ref(), &[1, 2, 3]);
        assert_eq!(part.content_type(), Some("application/octet-stream"));
    }

    #[test]
    fn part_file() {
        let part = Part::file("upload", "photo.jpg", vec![0xFF, 0xD8, 0xFF]);
        assert_eq!(part.name(), "upload");
        assert_eq!(part.filename(), Some("photo.jpg"));
        assert_eq!(part.content_type(), Some("image/jpeg"));
    }

    #[test]
    fn part_with_modifiers() {
        let part = Part::new("field", "data")
            .with_filename("custom.bin")
            .with_content_type("application/custom");
        assert_eq!(part.filename(), Some("custom.bin"));
        assert_eq!(part.content_type(), Some("application/custom"));
    }

    #[test]
    fn form_empty() {
        let form = Form::new();
        assert!(form.parts().is_empty());
        assert!(form.boundary().starts_with("----PincerBoundary"));
    }

    #[test]
    fn form_with_parts() {
        let form = Form::new().text("name", "John").file(
            "avatar",
            "photo.png",
            vec![0x89, 0x50, 0x4E, 0x47],
        );

        assert_eq!(form.parts().len(), 2);
        assert_eq!(form.parts().first().expect("part 0").name(), "name");
        assert_eq!(form.parts().get(1).expect("part 1").name(), "avatar");
    }

    #[test]
    fn form_content_type() {
        let form = Form::with_boundary("test-boundary");
        assert_eq!(
            form.content_type(),
            "multipart/form-data; boundary=test-boundary"
        );
    }

    #[test]
    fn form_encode() {
        let form = Form::with_boundary("boundary123").text("field", "value");

        let (content_type, body) = form.into_body();

        assert_eq!(content_type, "multipart/form-data; boundary=boundary123");

        let body_str = String::from_utf8_lossy(&body);
        assert!(body_str.contains("--boundary123\r\n"));
        assert!(body_str.contains("Content-Disposition: form-data; name=\"field\"\r\n"));
        assert!(body_str.contains("value\r\n"));
        assert!(body_str.contains("--boundary123--\r\n"));
    }

    #[test]
    fn form_encode_with_file() {
        let form = Form::with_boundary("boundary456").file("upload", "test.txt", "file content");

        let (_, body) = form.into_body();
        let body_str = String::from_utf8_lossy(&body);

        assert!(body_str.contains("name=\"upload\"; filename=\"test.txt\""));
        assert!(body_str.contains("Content-Type: text/plain\r\n"));
        assert!(body_str.contains("file content\r\n"));
    }

    #[test]
    fn guess_content_type_common() {
        assert_eq!(guess_content_type("photo.jpg"), "image/jpeg");
        assert_eq!(guess_content_type("photo.jpeg"), "image/jpeg");
        assert_eq!(guess_content_type("image.png"), "image/png");
        assert_eq!(guess_content_type("doc.pdf"), "application/pdf");
        assert_eq!(guess_content_type("data.json"), "application/json");
        assert_eq!(
            guess_content_type("unknown.xyz"),
            "application/octet-stream"
        );
    }

    #[test]
    fn guess_content_type_case_insensitive() {
        assert_eq!(guess_content_type("PHOTO.JPG"), "image/jpeg");
        assert_eq!(guess_content_type("Image.PNG"), "image/png");
    }
}
