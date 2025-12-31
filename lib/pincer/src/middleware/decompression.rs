//! Response decompression middleware.
//!
//! This middleware automatically decompresses HTTP responses that have been
//! compressed with gzip, deflate, br (brotli), or zstd.
//!
//! It adds the `Accept-Encoding` header to requests and decompresses responses
//! based on their `Content-Encoding` header.

use std::future::Future;
use std::io::Read;
use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::Bytes;
use tower::{Layer, Service};

use crate::{Error, Request, Response, Result};

/// Layer that enables automatic response decompression.
///
/// # Example
///
/// ```ignore
/// use pincer::middleware::DecompressionLayer;
/// use tower::ServiceBuilder;
///
/// let service = ServiceBuilder::new()
///     .layer(DecompressionLayer::new())
///     .service(client);
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct DecompressionLayer {
    _private: (),
}

impl DecompressionLayer {
    /// Create a new decompression layer.
    #[must_use]
    pub fn new() -> Self {
        Self { _private: () }
    }
}

impl<S> Layer<S> for DecompressionLayer {
    type Service = Decompression<S>;

    fn layer(&self, inner: S) -> Self::Service {
        Decompression { inner }
    }
}

/// Service that automatically decompresses HTTP responses.
#[derive(Debug, Clone)]
pub struct Decompression<S> {
    inner: S,
}

impl<S> Decompression<S> {
    /// Create a new decompression service wrapping the given service.
    #[must_use]
    pub fn new(inner: S) -> Self {
        Self { inner }
    }
}

/// Decompress bytes based on encoding.
fn decompress(encoding: &str, body: Bytes) -> Result<Bytes> {
    let result = match encoding {
        "gzip" | "x-gzip" => {
            let mut decoder = flate2::read::GzDecoder::new(body.as_ref());
            let mut decompressed = Vec::new();
            decoder
                .read_to_end(&mut decompressed)
                .map_err(|e| Error::InvalidRequest(format!("gzip decompression failed: {e}")))?;
            Bytes::from(decompressed)
        }
        "deflate" => {
            let mut decoder = flate2::read::DeflateDecoder::new(body.as_ref());
            let mut decompressed = Vec::new();
            decoder
                .read_to_end(&mut decompressed)
                .map_err(|e| Error::InvalidRequest(format!("deflate decompression failed: {e}")))?;
            Bytes::from(decompressed)
        }
        "br" => {
            let mut decompressed = Vec::new();
            brotli::BrotliDecompress(&mut body.as_ref(), &mut decompressed)
                .map_err(|e| Error::InvalidRequest(format!("brotli decompression failed: {e}")))?;
            Bytes::from(decompressed)
        }
        "zstd" => {
            let decompressed = zstd::decode_all(body.as_ref())
                .map_err(|e| Error::InvalidRequest(format!("zstd decompression failed: {e}")))?;
            Bytes::from(decompressed)
        }
        "identity" | "" => body,
        _ => {
            // Unknown encoding, return as-is
            body
        }
    };

    Ok(result)
}

impl<S> Service<Request<Bytes>> for Decompression<S>
where
    S: Service<Request<Bytes>, Response = Response<Bytes>, Error = Error> + Clone + Send + 'static,
    S::Future: Send,
{
    type Response = Response<Bytes>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<()>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: Request<Bytes>) -> Self::Future {
        // Add Accept-Encoding header if not present
        let request = if request.headers().contains_key("accept-encoding") {
            request
        } else {
            let (method, url, mut headers, body) = request.into_parts();
            headers.insert(
                "accept-encoding".to_string(),
                "gzip, deflate, br, zstd".to_string(),
            );
            Request::builder(method, url)
                .headers(headers)
                .body(body.unwrap_or_default())
                .build()
        };

        let mut inner = self.inner.clone();

        Box::pin(async move {
            let response = inner.call(request).await?;

            // Check for Content-Encoding header
            let encoding = response
                .headers()
                .get("content-encoding")
                .cloned()
                .unwrap_or_default();

            if encoding.is_empty() || encoding == "identity" {
                return Ok(response);
            }

            // Decompress the body
            let (status, mut headers, body) = response.into_parts();
            let decompressed = decompress(&encoding, body)?;

            // Remove Content-Encoding header since we've decompressed
            headers.remove("content-encoding");
            // Update Content-Length to reflect decompressed size
            headers.insert("content-length".to_string(), decompressed.len().to_string());

            Ok(Response::new(status, headers, decompressed))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decompression_layer_clone() {
        let layer = DecompressionLayer::new();
        let _ = layer;
    }

    #[test]
    fn decompression_layer_default() {
        let _layer = DecompressionLayer::default();
    }

    #[test]
    fn decompress_identity() {
        let body = Bytes::from("hello world");
        let result = decompress("identity", body.clone()).expect("decompress");
        assert_eq!(result, body);
    }

    #[test]
    fn decompress_empty_encoding() {
        let body = Bytes::from("hello world");
        let result = decompress("", body.clone()).expect("decompress");
        assert_eq!(result, body);
    }

    #[test]
    fn decompress_unknown_encoding() {
        let body = Bytes::from("hello world");
        let result = decompress("unknown", body.clone()).expect("decompress");
        assert_eq!(result, body);
    }

    #[test]
    fn decompress_gzip() {
        use flate2::Compression;
        use flate2::write::GzEncoder;
        use std::io::Write;

        let original = b"hello world";
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(original).expect("write");
        let compressed = encoder.finish().expect("finish");

        let result = decompress("gzip", Bytes::from(compressed)).expect("decompress");
        assert_eq!(result.as_ref(), original);
    }

    #[test]
    fn decompress_deflate() {
        use flate2::Compression;
        use flate2::write::DeflateEncoder;
        use std::io::Write;

        let original = b"hello world";
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(original).expect("write");
        let compressed = encoder.finish().expect("finish");

        let result = decompress("deflate", Bytes::from(compressed)).expect("decompress");
        assert_eq!(result.as_ref(), original);
    }

    #[test]
    fn decompress_brotli() {
        let original = b"hello world";
        let mut compressed = Vec::new();
        let params = brotli::enc::BrotliEncoderParams {
            quality: 4,
            ..Default::default()
        };
        brotli::BrotliCompress(&mut original.as_ref(), &mut compressed, &params).expect("compress");

        let result = decompress("br", Bytes::from(compressed)).expect("decompress");
        assert_eq!(result.as_ref(), original);
    }

    #[test]
    fn decompress_zstd() {
        let original = b"hello world";
        let compressed = zstd::encode_all(original.as_ref(), 3).expect("compress");

        let result = decompress("zstd", Bytes::from(compressed)).expect("decompress");
        assert_eq!(result.as_ref(), original);
    }
}
