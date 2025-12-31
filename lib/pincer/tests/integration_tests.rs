//! Integration tests for `HyperClient` using wiremock.

use pincer::{HttpClient, HyperClient, Method, Request};
use serde::{Deserialize, Serialize};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{body_json, header, method, path},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct User {
    id: u64,
    name: String,
}

#[tokio::test]
async fn test_get_request() {
    // Start mock server
    let mock_server = MockServer::start().await;

    let user = User {
        id: 1,
        name: "Alice".to_string(),
    };

    Mock::given(method("GET"))
        .and(path("/users/1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&user))
        .mount(&mock_server)
        .await;

    // Create client and execute request
    let client = HyperClient::new();
    let url = url::Url::parse(&format!("{}/users/1", mock_server.uri())).expect("url");
    let request = Request::builder(Method::Get, url)
        .header("Accept", "application/json")
        .build();

    let response = client.execute(request).await.expect("response");

    assert!(response.is_success());
    assert_eq!(response.status(), 200);

    let body: User = response.json().expect("json");
    assert_eq!(body, user);
}

#[tokio::test]
async fn test_post_request_with_json_body() {
    let mock_server = MockServer::start().await;

    let input = User {
        id: 0,
        name: "Bob".to_string(),
    };
    let output = User {
        id: 42,
        name: "Bob".to_string(),
    };

    Mock::given(method("POST"))
        .and(path("/users"))
        .and(header("Content-Type", "application/json"))
        .and(body_json(&input))
        .respond_with(ResponseTemplate::new(201).set_body_json(&output))
        .mount(&mock_server)
        .await;

    let client = HyperClient::new();
    let url = url::Url::parse(&format!("{}/users", mock_server.uri())).expect("url");
    let request = Request::builder(Method::Post, url)
        .json(&input)
        .expect("json body")
        .build();

    let response = client.execute(request).await.expect("response");

    assert!(response.is_success());
    assert_eq!(response.status(), 201);

    let body: User = response.json().expect("json");
    assert_eq!(body, output);
}

#[tokio::test]
async fn test_http_error_status() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/not-found"))
        .respond_with(ResponseTemplate::new(404).set_body_string("Not Found"))
        .mount(&mock_server)
        .await;

    let client = HyperClient::new();
    let url = url::Url::parse(&format!("{}/not-found", mock_server.uri())).expect("url");
    let request = Request::builder(Method::Get, url).build();

    let response = client.execute(request).await.expect("response");

    assert!(response.is_client_error());
    assert_eq!(response.status(), 404);
}

#[tokio::test]
async fn test_query_parameters() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/search"))
        .and(wiremock::matchers::query_param("q", "rust"))
        .and(wiremock::matchers::query_param("page", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "results": ["pincer", "rustls"]
        })))
        .mount(&mock_server)
        .await;

    let client = HyperClient::new();
    let url = url::Url::parse(&format!("{}/search", mock_server.uri())).expect("url");
    let request = Request::builder(Method::Get, url)
        .query("q", "rust")
        .query("page", "1")
        .build();

    let response = client.execute(request).await.expect("response");

    assert!(response.is_success());
}

#[tokio::test]
async fn test_custom_headers() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/data"))
        .and(header("Authorization", "Bearer token123"))
        .and(header("X-Custom-Header", "custom-value"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock_server)
        .await;

    let client = HyperClient::new();
    let url = url::Url::parse(&format!("{}/api/data", mock_server.uri())).expect("url");
    let request = Request::builder(Method::Get, url)
        .header("Authorization", "Bearer token123")
        .header("X-Custom-Header", "custom-value")
        .build();

    let response = client.execute(request).await.expect("response");

    assert!(response.is_success());
}

#[tokio::test]
async fn test_timeout() {
    let mock_server = MockServer::start().await;

    // Delay longer than client timeout
    Mock::given(method("GET"))
        .and(path("/slow"))
        .respond_with(ResponseTemplate::new(200).set_delay(std::time::Duration::from_secs(5)))
        .mount(&mock_server)
        .await;

    let client = HyperClient::builder()
        .timeout(std::time::Duration::from_millis(100))
        .build();

    let url = url::Url::parse(&format!("{}/slow", mock_server.uri())).expect("url");
    let request = Request::builder(Method::Get, url).build();

    let result = client.execute(request).await;

    let err = result.expect_err("expected timeout error");
    assert!(err.is_timeout(), "Expected timeout error, got: {err}");
}

#[tokio::test]
async fn test_connection_error() {
    let client = HyperClient::new();

    // Try to connect to a non-existent server
    let url = url::Url::parse("http://127.0.0.1:1").expect("url");
    let request = Request::builder(Method::Get, url).build();

    let result = client.execute(request).await;

    let err = result.expect_err("expected connection error");
    assert!(err.is_connection(), "Expected connection error, got: {err}");
}

#[tokio::test]
async fn test_response_headers() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/with-headers"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("X-Request-Id", "abc123")
                .insert_header("Content-Type", "application/json")
                .set_body_json(serde_json::json!({"ok": true})),
        )
        .mount(&mock_server)
        .await;

    let client = HyperClient::new();
    let url = url::Url::parse(&format!("{}/with-headers", mock_server.uri())).expect("url");
    let request = Request::builder(Method::Get, url).build();

    let response = client.execute(request).await.expect("response");

    assert_eq!(response.header("x-request-id"), Some("abc123"));
    assert_eq!(response.header("content-type"), Some("application/json"));
}

#[tokio::test]
async fn test_put_request() {
    let mock_server = MockServer::start().await;

    let user = User {
        id: 1,
        name: "Updated".to_string(),
    };

    Mock::given(method("PUT"))
        .and(path("/users/1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&user))
        .mount(&mock_server)
        .await;

    let client = HyperClient::new();
    let url = url::Url::parse(&format!("{}/users/1", mock_server.uri())).expect("url");
    let request = Request::builder(Method::Put, url)
        .json(&user)
        .expect("json")
        .build();

    let response = client.execute(request).await.expect("response");

    assert!(response.is_success());
}

#[tokio::test]
async fn test_delete_request() {
    let mock_server = MockServer::start().await;

    Mock::given(method("DELETE"))
        .and(path("/users/1"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&mock_server)
        .await;

    let client = HyperClient::new();
    let url = url::Url::parse(&format!("{}/users/1", mock_server.uri())).expect("url");
    let request = Request::builder(Method::Delete, url).build();

    let response = client.execute(request).await.expect("response");

    assert!(response.is_success());
    assert_eq!(response.status(), 204);
}
