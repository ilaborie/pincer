//! HTTPS connector using rustls.

use hyper_rustls::{HttpsConnector, HttpsConnectorBuilder};
use hyper_util::client::legacy::connect::HttpConnector;

/// Create an HTTPS connector with rustls.
///
/// This connector supports both HTTP/1.1 and HTTP/2, with TLS enabled
/// using the Mozilla root certificates.
#[must_use]
pub fn https_connector() -> HttpsConnector<HttpConnector> {
    // Build rustls client config with webpki roots
    let root_store: rustls::RootCertStore =
        webpki_roots::TLS_SERVER_ROOTS.iter().cloned().collect();

    let tls_config = rustls::ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    HttpsConnectorBuilder::new()
        .with_tls_config(tls_config)
        .https_or_http()
        .enable_http1()
        .enable_http2()
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_connector() {
        let _connector = https_connector();
        // Just verify it compiles and doesn't panic
    }
}
