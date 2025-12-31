//! Integration tests for middleware functionality.

use pincer::{HttpClient, HyperClient, Method, Request};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{header, header_exists, method, path},
};

/// Test that bearer auth middleware adds Authorization header.
#[tokio::test]
async fn test_bearer_auth_header() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/protected"))
        .and(header("Authorization", "Bearer my-secret-token"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"user": "alice"})),
        )
        .mount(&mock_server)
        .await;

    let client = HyperClient::builder()
        .with_bearer_auth("my-secret-token")
        .build();

    let url = url::Url::parse(&format!("{}/protected", mock_server.uri())).expect("url");
    let request = Request::builder(Method::Get, url).build();

    let response = client.execute(request).await.expect("response");

    assert!(response.is_success());
}

/// Test that logging middleware doesn't break request/response flow.
#[tokio::test]
async fn test_logging_middleware() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/logged"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"logged": true})))
        .mount(&mock_server)
        .await;

    let client = HyperClient::builder().with_logging().build();

    let url = url::Url::parse(&format!("{}/logged", mock_server.uri())).expect("url");
    let request = Request::builder(Method::Get, url).build();

    let response = client.execute(request).await.expect("response");

    assert!(response.is_success());
}

/// Test multiple middleware composed together.
#[tokio::test]
async fn test_middleware_composition() {
    let mock_server = MockServer::start().await;

    // Require both Authorization header and return success
    Mock::given(method("GET"))
        .and(path("/composed"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"composed": true})),
        )
        .mount(&mock_server)
        .await;

    // Compose: logging -> bearer auth -> retry
    let client = HyperClient::builder()
        .with_logging()
        .with_bearer_auth("test-token")
        .with_retry(2)
        .build();

    let url = url::Url::parse(&format!("{}/composed", mock_server.uri())).expect("url");
    let request = Request::builder(Method::Get, url).build();

    let response = client.execute(request).await.expect("response");

    assert!(response.is_success());
}

/// Test that no retries happen for 4xx errors.
#[tokio::test]
async fn test_no_retry_on_client_error() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/not-found"))
        .respond_with(ResponseTemplate::new(404))
        .expect(1) // Should only be called once, no retries
        .mount(&mock_server)
        .await;

    let client = HyperClient::builder().with_retry(3).build();

    let url = url::Url::parse(&format!("{}/not-found", mock_server.uri())).expect("url");
    let request = Request::builder(Method::Get, url).build();

    let response = client.execute(request).await.expect("response");

    assert_eq!(response.status(), 404);
}

/// Test retry on server error (5xx).
#[tokio::test]
async fn test_retry_on_server_error() {
    let mock_server = MockServer::start().await;

    // Server always returns 503, retry should exhaust attempts
    Mock::given(method("GET"))
        .and(path("/error"))
        .respond_with(ResponseTemplate::new(503))
        .expect(3) // Initial + 2 retries
        .mount(&mock_server)
        .await;

    let client = HyperClient::builder().with_retry(2).build();

    let url = url::Url::parse(&format!("{}/error", mock_server.uri())).expect("url");
    let request = Request::builder(Method::Get, url).build();

    let response = client.execute(request).await.expect("response");

    // Should get the 503 after exhausting retries
    assert_eq!(response.status(), 503);
}

/// Test retry on 429 Too Many Requests.
#[tokio::test]
async fn test_retry_on_rate_limit() {
    let mock_server = MockServer::start().await;

    // Server always returns 429
    Mock::given(method("GET"))
        .and(path("/rate-limited"))
        .respond_with(ResponseTemplate::new(429))
        .expect(3) // Initial + 2 retries
        .mount(&mock_server)
        .await;

    let client = HyperClient::builder().with_retry(2).build();

    let url = url::Url::parse(&format!("{}/rate-limited", mock_server.uri())).expect("url");
    let request = Request::builder(Method::Get, url).build();

    let response = client.execute(request).await.expect("response");

    assert_eq!(response.status(), 429);
}

/// Test client builder with defaults.
#[tokio::test]
async fn test_client_with_defaults() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/default"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock_server)
        .await;

    let client = HyperClient::builder().with_defaults().build();

    let url = url::Url::parse(&format!("{}/default", mock_server.uri())).expect("url");
    let request = Request::builder(Method::Get, url).build();

    let response = client.execute(request).await.expect("response");

    assert!(response.is_success());
}

/// Test generic layer API with custom middleware.
#[tokio::test]
async fn test_generic_layer_api() {
    use pincer::middleware::BearerAuthLayer;

    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/custom-layer"))
        .and(header("Authorization", "Bearer custom-token"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock_server)
        .await;

    // Use generic .layer() API instead of helper
    let client = HyperClient::builder()
        .layer(BearerAuthLayer::new("custom-token"))
        .build();

    let url = url::Url::parse(&format!("{}/custom-layer", mock_server.uri())).expect("url");
    let request = Request::builder(Method::Get, url).build();

    let response = client.execute(request).await.expect("response");

    assert!(response.is_success());
}

/// Test debug logging level.
#[tokio::test]
async fn test_debug_logging() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/debug"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock_server)
        .await;

    let client = HyperClient::builder().with_debug_logging().build();

    let url = url::Url::parse(&format!("{}/debug", mock_server.uri())).expect("url");
    let request = Request::builder(Method::Get, url).build();

    let response = client.execute(request).await.expect("response");

    assert!(response.is_success());
}

/// Test that basic auth middleware adds Authorization header.
#[tokio::test]
async fn test_basic_auth_header() {
    let mock_server = MockServer::start().await;

    // "user:pass" base64 encoded is "dXNlcjpwYXNz"
    Mock::given(method("GET"))
        .and(path("/protected"))
        .and(header("Authorization", "Basic dXNlcjpwYXNz"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"user": "authenticated"})),
        )
        .mount(&mock_server)
        .await;

    let client = HyperClient::builder()
        .with_basic_auth("user", "pass")
        .build();

    let url = url::Url::parse(&format!("{}/protected", mock_server.uri())).expect("url");
    let request = Request::builder(Method::Get, url).build();

    let response = client.execute(request).await.expect("response");

    assert!(response.is_success());
}

// ============================================================================
// Follow Redirect Tests
// ============================================================================

/// Test that follow redirect middleware handles 302 redirect.
#[tokio::test]
async fn test_follow_redirect_302() {
    let mock_server = MockServer::start().await;

    // First request returns 302 with Location header
    Mock::given(method("GET"))
        .and(path("/old"))
        .respond_with(ResponseTemplate::new(302).insert_header("Location", "/new"))
        .mount(&mock_server)
        .await;

    // Redirect target returns 200
    Mock::given(method("GET"))
        .and(path("/new"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"status": "redirected"})),
        )
        .mount(&mock_server)
        .await;

    let client = HyperClient::builder().with_follow_redirects().build();

    let url = url::Url::parse(&format!("{}/old", mock_server.uri())).expect("url");
    let request = Request::builder(Method::Get, url).build();

    let response = client.execute(request).await.expect("response");

    assert!(response.is_success());
    assert_eq!(response.status(), 200);
}

/// Test that follow redirect middleware handles 301 permanent redirect.
#[tokio::test]
async fn test_follow_redirect_301() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/old-permanent"))
        .respond_with(ResponseTemplate::new(301).insert_header("Location", "/new-permanent"))
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/new-permanent"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock_server)
        .await;

    let client = HyperClient::builder().with_follow_redirects().build();

    let url = url::Url::parse(&format!("{}/old-permanent", mock_server.uri())).expect("url");
    let request = Request::builder(Method::Get, url).build();

    let response = client.execute(request).await.expect("response");

    assert!(response.is_success());
}

/// Test that follow redirect middleware respects max redirects limit.
#[tokio::test]
async fn test_follow_redirect_max_exceeded() {
    let mock_server = MockServer::start().await;

    // Create a redirect loop
    Mock::given(method("GET"))
        .and(path("/loop1"))
        .respond_with(ResponseTemplate::new(302).insert_header("Location", "/loop2"))
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/loop2"))
        .respond_with(ResponseTemplate::new(302).insert_header("Location", "/loop1"))
        .mount(&mock_server)
        .await;

    let client = HyperClient::builder().with_follow_redirects_max(3).build();

    let url = url::Url::parse(&format!("{}/loop1", mock_server.uri())).expect("url");
    let request = Request::builder(Method::Get, url).build();

    let result = client.execute(request).await;

    assert!(result.is_err());
    let err = result.expect_err("should fail");
    assert!(matches!(err, pincer::Error::TooManyRedirects { .. }));
}

/// Test that 307 redirect preserves POST method.
#[tokio::test]
async fn test_follow_redirect_307_preserves_method() {
    let mock_server = MockServer::start().await;

    // 307 should preserve method
    Mock::given(method("POST"))
        .and(path("/old-post"))
        .respond_with(ResponseTemplate::new(307).insert_header("Location", "/new-post"))
        .mount(&mock_server)
        .await;

    // Expect POST on redirect target (not GET)
    Mock::given(method("POST"))
        .and(path("/new-post"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock_server)
        .await;

    let client = HyperClient::builder().with_follow_redirects().build();

    let url = url::Url::parse(&format!("{}/old-post", mock_server.uri())).expect("url");
    let request = Request::builder(Method::Post, url)
        .body(bytes::Bytes::from("test body"))
        .build();

    let response = client.execute(request).await.expect("response");

    assert!(response.is_success());
}

/// Test that 302 redirect changes POST to GET.
#[tokio::test]
async fn test_follow_redirect_302_changes_post_to_get() {
    let mock_server = MockServer::start().await;

    // 302 should change POST to GET (browser behavior)
    Mock::given(method("POST"))
        .and(path("/submit"))
        .respond_with(ResponseTemplate::new(302).insert_header("Location", "/result"))
        .mount(&mock_server)
        .await;

    // Expect GET on redirect target (not POST)
    Mock::given(method("GET"))
        .and(path("/result"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock_server)
        .await;

    let client = HyperClient::builder().with_follow_redirects().build();

    let url = url::Url::parse(&format!("{}/submit", mock_server.uri())).expect("url");
    let request = Request::builder(Method::Post, url)
        .body(bytes::Bytes::from("form data"))
        .build();

    let response = client.execute(request).await.expect("response");

    assert!(response.is_success());
}

// ============================================================================
// Decompression Tests
// ============================================================================

/// Test that decompression middleware adds Accept-Encoding header.
#[tokio::test]
async fn test_decompression_adds_accept_encoding() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/compress"))
        .and(header_exists("accept-encoding"))
        .respond_with(ResponseTemplate::new(200).set_body_string("uncompressed"))
        .mount(&mock_server)
        .await;

    let client = HyperClient::builder().with_decompression().build();

    let url = url::Url::parse(&format!("{}/compress", mock_server.uri())).expect("url");
    let request = Request::builder(Method::Get, url).build();

    let response = client.execute(request).await.expect("response");

    assert!(response.is_success());
}

/// Test that decompression middleware handles gzip-encoded responses.
#[tokio::test]
async fn test_decompression_gzip() {
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use std::io::Write;

    let mock_server = MockServer::start().await;

    // Compress the response body
    let original = b"hello world from gzip!";
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(original).expect("write");
    let compressed = encoder.finish().expect("finish");

    Mock::given(method("GET"))
        .and(path("/gzipped"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-encoding", "gzip")
                .set_body_bytes(compressed),
        )
        .mount(&mock_server)
        .await;

    let client = HyperClient::builder().with_decompression().build();

    let url = url::Url::parse(&format!("{}/gzipped", mock_server.uri())).expect("url");
    let request = Request::builder(Method::Get, url).build();

    let response = client.execute(request).await.expect("response");

    assert!(response.is_success());
    let body = response.into_body();
    assert_eq!(body.as_ref(), original);
}

/// Test that decompression passes through uncompressed responses.
#[tokio::test]
async fn test_decompression_passthrough() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/plain"))
        .respond_with(ResponseTemplate::new(200).set_body_string("plain text"))
        .mount(&mock_server)
        .await;

    let client = HyperClient::builder().with_decompression().build();

    let url = url::Url::parse(&format!("{}/plain", mock_server.uri())).expect("url");
    let request = Request::builder(Method::Get, url).build();

    let response = client.execute(request).await.expect("response");

    assert!(response.is_success());
    let body = response.into_body();
    assert_eq!(body.as_ref(), b"plain text");
}
