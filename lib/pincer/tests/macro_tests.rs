//! Integration tests for pincer proc-macros.

#![allow(missing_docs)]

use pincer::prelude::*;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{header, method, path, query_param},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct User {
    id: u64,
    name: String,
}

// Test the #[pincer] attribute generates a proper client struct from a trait
#[pincer(url = "https://api.example.com")]
pub trait TestApi {
    #[get("/health")]
    async fn health(&self) -> pincer::Result<()>;
}

#[tokio::test]
async fn test_pincer_generates_builder() {
    // The #[pincer] macro should generate a builder via the generated client struct
    let client = TestApiClientBuilder::default()
        .base_url("http://localhost:8080")
        .build()
        .expect("build client");

    // Verify the base_url method exists
    assert_eq!(client.base_url().as_str(), "http://localhost:8080/");
}

#[tokio::test]
async fn test_pincer_default_base_url() {
    let client = TestApiClientBuilder::default()
        .build()
        .expect("build client");

    // Default base URL from the #[pincer] attribute
    assert_eq!(client.base_url().as_str(), "https://api.example.com/");
}

// Test a complete client with methods using wiremock
#[pincer(url = "http://localhost:9999")]
pub trait UserApi {
    #[get("/users/{id}")]
    async fn get_user(&self, #[path] id: u64) -> pincer::Result<User>;

    #[http("GET /users")]
    async fn list_users(&self) -> pincer::Result<Vec<User>>;
}

#[tokio::test]
async fn test_get_method_with_path_param() {
    let mock_server = MockServer::start().await;

    let user = User {
        id: 42,
        name: "Alice".to_string(),
    };

    Mock::given(method("GET"))
        .and(path("/users/42"))
        .and(header("Accept", "application/json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&user))
        .mount(&mock_server)
        .await;

    let client = UserApiClientBuilder::default()
        .base_url(mock_server.uri())
        .build()
        .expect("build client");

    let result = client.get_user(42).await.expect("get user");
    assert_eq!(result, user);
}

#[tokio::test]
async fn test_http_method_attribute() {
    let mock_server = MockServer::start().await;

    let users = vec![
        User {
            id: 1,
            name: "Alice".to_string(),
        },
        User {
            id: 2,
            name: "Bob".to_string(),
        },
    ];

    Mock::given(method("GET"))
        .and(path("/users"))
        .and(header("Accept", "application/json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&users))
        .mount(&mock_server)
        .await;

    let client = UserApiClientBuilder::default()
        .base_url(mock_server.uri())
        .build()
        .expect("build client");

    let result = client.list_users().await.expect("list users");
    assert_eq!(result, users);
}

// ============================================================================
// Tests for new features: path encoding, Vec<T> query, struct query
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct SearchResult {
    query: String,
    count: u32,
}

/// Query params struct for struct-based query serialization
#[derive(Debug, Clone, Serialize, Query)]
struct SearchParams {
    q: String,
    page: Option<u32>,
}

#[pincer(url = "http://localhost:9999")]
pub trait AdvancedApi {
    /// Test path parameter with special characters (spaces, etc.)
    #[get("/search/{query}")]
    async fn search_with_path(&self, #[path] query: &str) -> pincer::Result<SearchResult>;

    /// Test Vec<T> query parameters
    #[get("/filter")]
    async fn filter_with_tags(&self, #[query] tags: Vec<String>) -> pincer::Result<SearchResult>;

    /// Test struct-based query parameters
    #[get("/search")]
    async fn search_with_struct(
        &self,
        #[query] params: &SearchParams,
    ) -> pincer::Result<SearchResult>;

    /// Test Option<T> query parameters
    #[get("/items")]
    async fn get_items(
        &self,
        #[query] page: Option<u32>,
        #[query] limit: Option<u32>,
    ) -> pincer::Result<Vec<User>>;

    /// Test header map for dynamic headers
    #[get("/with-headers")]
    async fn get_with_headers(
        &self,
        #[headers] extra_headers: std::collections::HashMap<String, String>,
    ) -> pincer::Result<SearchResult>;
}

#[tokio::test]
async fn test_path_param_with_special_chars() {
    let mock_server = MockServer::start().await;

    let result = SearchResult {
        query: "hello world".to_string(),
        count: 42,
    };

    // Path should be URL-encoded: "hello%20world"
    Mock::given(method("GET"))
        .and(path("/search/hello%20world"))
        .and(header("Accept", "application/json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&result))
        .mount(&mock_server)
        .await;

    let client = AdvancedApiClientBuilder::default()
        .base_url(mock_server.uri())
        .build()
        .expect("build client");

    let response = client
        .search_with_path("hello world")
        .await
        .expect("search");
    assert_eq!(response.query, "hello world");
    assert_eq!(response.count, 42);
}

#[tokio::test]
async fn test_vec_query_params() {
    let mock_server = MockServer::start().await;

    let result = SearchResult {
        query: "tags".to_string(),
        count: 3,
    };

    // Vec<String> should produce repeated params: ?tags=rust&tags=http&tags=async
    Mock::given(method("GET"))
        .and(path("/filter"))
        .and(query_param("tags", "rust"))
        .and(query_param("tags", "http"))
        .and(query_param("tags", "async"))
        .and(header("Accept", "application/json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&result))
        .mount(&mock_server)
        .await;

    let client = AdvancedApiClientBuilder::default()
        .base_url(mock_server.uri())
        .build()
        .expect("build client");

    let response = client
        .filter_with_tags(vec![
            "rust".to_string(),
            "http".to_string(),
            "async".to_string(),
        ])
        .await
        .expect("filter");
    assert_eq!(response.count, 3);
}

#[tokio::test]
async fn test_struct_query_params() {
    let mock_server = MockServer::start().await;

    let result = SearchResult {
        query: "rust".to_string(),
        count: 100,
    };

    // Struct should be serialized to query params
    Mock::given(method("GET"))
        .and(path("/search"))
        .and(query_param("q", "rust"))
        .and(query_param("page", "2"))
        .and(header("Accept", "application/json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&result))
        .mount(&mock_server)
        .await;

    let client = AdvancedApiClientBuilder::default()
        .base_url(mock_server.uri())
        .build()
        .expect("build client");

    let params = SearchParams {
        q: "rust".to_string(),
        page: Some(2),
    };

    let response = client.search_with_struct(&params).await.expect("search");
    assert_eq!(response.query, "rust");
    assert_eq!(response.count, 100);
}

#[tokio::test]
async fn test_struct_query_params_with_none() {
    let mock_server = MockServer::start().await;

    let result = SearchResult {
        query: "rust".to_string(),
        count: 50,
    };

    // When page is None, it should be skipped (via serde skip_serializing_if)
    Mock::given(method("GET"))
        .and(path("/search"))
        .and(query_param("q", "rust"))
        .and(header("Accept", "application/json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&result))
        .mount(&mock_server)
        .await;

    let client = AdvancedApiClientBuilder::default()
        .base_url(mock_server.uri())
        .build()
        .expect("build client");

    let params = SearchParams {
        q: "rust".to_string(),
        page: None,
    };

    let response = client.search_with_struct(&params).await.expect("search");
    assert_eq!(response.query, "rust");
}

#[tokio::test]
async fn test_option_query_params() {
    let mock_server = MockServer::start().await;

    let users = vec![User {
        id: 1,
        name: "Test".to_string(),
    }];

    // Only page should be present, limit is None
    Mock::given(method("GET"))
        .and(path("/items"))
        .and(query_param("page", "1"))
        .and(header("Accept", "application/json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&users))
        .mount(&mock_server)
        .await;

    let client = AdvancedApiClientBuilder::default()
        .base_url(mock_server.uri())
        .build()
        .expect("build client");

    let response = client.get_items(Some(1), None).await.expect("get items");
    assert_eq!(response.len(), 1);
}

#[tokio::test]
async fn test_header_map() {
    let mock_server = MockServer::start().await;

    let result = SearchResult {
        query: "headers".to_string(),
        count: 1,
    };

    // Test that custom headers from a HashMap are sent
    Mock::given(method("GET"))
        .and(path("/with-headers"))
        .and(header("Accept", "application/json"))
        .and(header("X-Custom-Header", "custom-value"))
        .and(header("X-Request-Id", "12345"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&result))
        .mount(&mock_server)
        .await;

    let client = AdvancedApiClientBuilder::default()
        .base_url(mock_server.uri())
        .build()
        .expect("build client");

    let mut headers = std::collections::HashMap::new();
    headers.insert("X-Custom-Header".to_string(), "custom-value".to_string());
    headers.insert("X-Request-Id".to_string(), "12345".to_string());

    let response = client
        .get_with_headers(headers)
        .await
        .expect("get with headers");
    assert_eq!(response.query, "headers");
}

// ============================================================================
// Tests for trait-level headers
// ============================================================================

#[pincer(url = "http://localhost:9999")]
#[headers(X_Api_Version = "v1", X_Custom_Header = "static-value")]
pub trait TraitHeadersApi {
    #[get("/data")]
    async fn get_data(&self) -> pincer::Result<SearchResult>;

    /// Method-level headers should be added alongside trait-level headers
    #[get("/data-with-auth")]
    async fn get_data_with_auth(
        &self,
        #[header("Authorization")] token: &str,
    ) -> pincer::Result<SearchResult>;
}

#[tokio::test]
async fn test_trait_level_headers() {
    let mock_server = MockServer::start().await;

    let result = SearchResult {
        query: "headers".to_string(),
        count: 1,
    };

    // Verify trait-level headers are sent
    Mock::given(method("GET"))
        .and(path("/data"))
        .and(header("Accept", "application/json"))
        .and(header("X-Api-Version", "v1"))
        .and(header("X-Custom-Header", "static-value"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&result))
        .mount(&mock_server)
        .await;

    let client = TraitHeadersApiClientBuilder::default()
        .base_url(mock_server.uri())
        .build()
        .expect("build client");

    let response = client.get_data().await.expect("get data");
    assert_eq!(response.query, "headers");
}

#[tokio::test]
async fn test_trait_level_headers_combined_with_method_headers() {
    let mock_server = MockServer::start().await;

    let result = SearchResult {
        query: "combined".to_string(),
        count: 2,
    };

    // Verify both trait-level and method-level headers are sent
    Mock::given(method("GET"))
        .and(path("/data-with-auth"))
        .and(header("Accept", "application/json"))
        .and(header("X-Api-Version", "v1"))
        .and(header("X-Custom-Header", "static-value"))
        .and(header("Authorization", "Bearer my-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&result))
        .mount(&mock_server)
        .await;

    let client = TraitHeadersApiClientBuilder::default()
        .base_url(mock_server.uri())
        .build()
        .expect("build client");

    let response = client
        .get_data_with_auth("Bearer my-token")
        .await
        .expect("get data with auth");
    assert_eq!(response.query, "combined");
}

// ============================================================================
// Tests for Phase 1 features: not_found_as_none, timeout, decode_body
// ============================================================================

/// Error response body for testing `decode_body()`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct ApiError {
    code: String,
    message: String,
}

#[pincer(url = "http://localhost:9999")]
pub trait ErrorHandlingApi {
    /// Test `#[not_found_as_none]`: 404 returns `Ok(None)` instead of error.
    #[get("/users/{id}")]
    #[not_found_as_none]
    async fn find_user(&self, #[path] id: u64) -> pincer::Result<Option<User>>;

    /// Test per-method timeout attribute.
    #[get("/slow")]
    #[timeout("1s")]
    async fn slow_endpoint(&self) -> pincer::Result<User>;

    /// Test error body preservation for `decode_body()`.
    #[get("/api/resource/{id}")]
    async fn get_resource(&self, #[path] id: u64) -> pincer::Result<User>;
}

#[tokio::test]
async fn test_not_found_as_none_returns_none_on_404() {
    let mock_server = MockServer::start().await;

    // 404 response
    Mock::given(method("GET"))
        .and(path("/users/999"))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
            "code": "NOT_FOUND",
            "message": "User not found"
        })))
        .mount(&mock_server)
        .await;

    let client = ErrorHandlingApiClientBuilder::default()
        .base_url(mock_server.uri())
        .build()
        .expect("build client");

    let result = client
        .find_user(999)
        .await
        .expect("should not error on 404");
    assert!(result.is_none(), "404 should return None");
}

#[tokio::test]
async fn test_not_found_as_none_returns_some_on_success() {
    let mock_server = MockServer::start().await;

    let user = User {
        id: 42,
        name: "Alice".to_string(),
    };

    Mock::given(method("GET"))
        .and(path("/users/42"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&user))
        .mount(&mock_server)
        .await;

    let client = ErrorHandlingApiClientBuilder::default()
        .base_url(mock_server.uri())
        .build()
        .expect("build client");

    let result = client.find_user(42).await.expect("should succeed");
    assert_eq!(result, Some(user));
}

#[tokio::test]
async fn test_not_found_as_none_errors_on_other_status() {
    let mock_server = MockServer::start().await;

    // 500 error should still be an error
    Mock::given(method("GET"))
        .and(path("/users/123"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .mount(&mock_server)
        .await;

    let client = ErrorHandlingApiClientBuilder::default()
        .base_url(mock_server.uri())
        .build()
        .expect("build client");

    let result = client.find_user(123).await;
    let err = result.expect_err("500 should still be an error");
    assert!(err.is_server_error(), "should be server error");
}

#[tokio::test]
async fn test_timeout_attribute() {
    let mock_server = MockServer::start().await;

    let user = User {
        id: 1,
        name: "Slow User".to_string(),
    };

    // Delay longer than the 1s timeout
    Mock::given(method("GET"))
        .and(path("/slow"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(&user)
                .set_delay(std::time::Duration::from_secs(3)),
        )
        .mount(&mock_server)
        .await;

    let client = ErrorHandlingApiClientBuilder::default()
        .base_url(mock_server.uri())
        .build()
        .expect("build client");

    let result = client.slow_endpoint().await;
    let err = result.expect_err("should timeout");
    assert!(err.is_timeout(), "should be timeout error");
}

#[tokio::test]
async fn test_error_body_preserved_for_decode_body() {
    let mock_server = MockServer::start().await;

    let api_error = serde_json::json!({
        "code": "VALIDATION_ERROR",
        "message": "Invalid resource ID"
    });

    Mock::given(method("GET"))
        .and(path("/api/resource/0"))
        .respond_with(ResponseTemplate::new(400).set_body_json(&api_error))
        .mount(&mock_server)
        .await;

    let client = ErrorHandlingApiClientBuilder::default()
        .base_url(mock_server.uri())
        .build()
        .expect("build client");

    let result = client.get_resource(0).await;
    let err = result.expect_err("should error on 400");
    assert!(err.is_client_error(), "should be client error");
    assert!(err.body().is_some(), "error should have body");

    // Test decode_body()
    let api_error = err
        .decode_body::<ApiError>()
        .expect("should be able to decode body")
        .expect("decode should succeed");
    assert_eq!(api_error.code, "VALIDATION_ERROR");
    assert_eq!(api_error.message, "Invalid resource ID");
}

// ============================================================================
// Tests for raw Response<Bytes> and unit Result<()> return types
// ============================================================================

#[pincer(url = "http://localhost:9999")]
pub trait ReturnTypesApi {
    /// Test raw Response<Bytes> return - no JSON deserialization
    #[get("/raw")]
    async fn get_raw(&self) -> pincer::Result<pincer::Response<bytes::Bytes>>;

    /// Test raw Response with `not_found_as_none`
    #[get("/raw/{id}")]
    #[not_found_as_none]
    async fn get_raw_optional(
        &self,
        #[path] id: u64,
    ) -> pincer::Result<Option<pincer::Response<bytes::Bytes>>>;

    /// Test unit return type Result<()>
    #[delete("/items/{id}")]
    async fn delete_item(&self, #[path] id: u64) -> pincer::Result<()>;

    /// Test unit return type with `not_found_as_none`
    #[delete("/items/{id}/soft")]
    #[not_found_as_none]
    async fn soft_delete_item(&self, #[path] id: u64) -> pincer::Result<Option<()>>;
}

#[tokio::test]
async fn test_raw_response_return_type() {
    let mock_server = MockServer::start().await;

    // Return plain text (not JSON)
    Mock::given(method("GET"))
        .and(path("/raw"))
        .respond_with(ResponseTemplate::new(200).set_body_string("Hello, raw world!"))
        .mount(&mock_server)
        .await;

    let client = ReturnTypesApiClientBuilder::default()
        .base_url(mock_server.uri())
        .build()
        .expect("build client");

    let response = client.get_raw().await.expect("should succeed");
    assert!(response.is_success());
    assert_eq!(response.status(), 200);
    assert_eq!(response.text().expect("text"), "Hello, raw world!");
}

#[tokio::test]
async fn test_raw_response_with_error_status() {
    let mock_server = MockServer::start().await;

    // Return 500 error - raw response should still return it without error
    Mock::given(method("GET"))
        .and(path("/raw"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .mount(&mock_server)
        .await;

    let client = ReturnTypesApiClientBuilder::default()
        .base_url(mock_server.uri())
        .build()
        .expect("build client");

    // Raw response returns the response even for error status codes
    let response = client.get_raw().await.expect("should succeed");
    assert!(!response.is_success());
    assert_eq!(response.status(), 500);
    assert_eq!(response.text().expect("text"), "Internal Server Error");
}

#[tokio::test]
async fn test_raw_response_with_not_found_as_none() {
    let mock_server = MockServer::start().await;

    // 404 should return None
    Mock::given(method("GET"))
        .and(path("/raw/999"))
        .respond_with(ResponseTemplate::new(404).set_body_string("Not Found"))
        .mount(&mock_server)
        .await;

    let client = ReturnTypesApiClientBuilder::default()
        .base_url(mock_server.uri())
        .build()
        .expect("build client");

    let result = client
        .get_raw_optional(999)
        .await
        .expect("should not error");
    assert!(result.is_none());
}

#[tokio::test]
async fn test_unit_return_type() {
    let mock_server = MockServer::start().await;

    Mock::given(method("DELETE"))
        .and(path("/items/42"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&mock_server)
        .await;

    let client = ReturnTypesApiClientBuilder::default()
        .base_url(mock_server.uri())
        .build()
        .expect("build client");

    // Should succeed with no return value
    client.delete_item(42).await.expect("should succeed");
}

#[tokio::test]
async fn test_unit_return_type_error_on_failure() {
    let mock_server = MockServer::start().await;

    Mock::given(method("DELETE"))
        .and(path("/items/42"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Server Error"))
        .mount(&mock_server)
        .await;

    let client = ReturnTypesApiClientBuilder::default()
        .base_url(mock_server.uri())
        .build()
        .expect("build client");

    let result = client.delete_item(42).await;
    let err = result.expect_err("should error on 500");
    assert!(err.is_server_error());
}

#[tokio::test]
async fn test_unit_return_type_with_not_found_as_none() {
    let mock_server = MockServer::start().await;

    // 404 should return Ok(None)
    Mock::given(method("DELETE"))
        .and(path("/items/999/soft"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mock_server)
        .await;

    // 204 should return Ok(Some(()))
    Mock::given(method("DELETE"))
        .and(path("/items/42/soft"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&mock_server)
        .await;

    let client = ReturnTypesApiClientBuilder::default()
        .base_url(mock_server.uri())
        .build()
        .expect("build client");

    // 404 returns None
    let result = client
        .soft_delete_item(999)
        .await
        .expect("should not error");
    assert!(result.is_none());

    // 204 returns Some(())
    let result = client.soft_delete_item(42).await.expect("should succeed");
    assert!(result.is_some());
}

// ============================================================================
// Tests for collection format query parameters
// ============================================================================

#[pincer(url = "http://localhost:9999")]
pub trait CollectionFormatApi {
    /// Default format (multi): ?tags=a&tags=b&tags=c
    #[get("/search")]
    async fn search_multi(&self, #[query] tags: Vec<String>) -> pincer::Result<SearchResult>;

    /// CSV format: ?tags=a,b,c
    #[get("/search-csv")]
    async fn search_csv(
        &self,
        #[query(format = "csv")] tags: Vec<String>,
    ) -> pincer::Result<SearchResult>;

    /// SSV (space-separated) format: ?tags=a%20b%20c
    #[get("/search-ssv")]
    async fn search_ssv(
        &self,
        #[query(format = "ssv")] tags: Vec<String>,
    ) -> pincer::Result<SearchResult>;

    /// Pipes format: ?tags=a|b|c
    #[get("/search-pipes")]
    async fn search_pipes(
        &self,
        #[query(format = "pipes")] tags: Vec<String>,
    ) -> pincer::Result<SearchResult>;
}

#[tokio::test]
async fn test_collection_format_csv() {
    let mock_server = MockServer::start().await;

    let result = SearchResult {
        query: "csv".to_string(),
        count: 3,
    };

    // CSV format should produce a single param with comma-separated values
    Mock::given(method("GET"))
        .and(path("/search-csv"))
        .and(query_param("tags", "rust,http,async"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&result))
        .mount(&mock_server)
        .await;

    let client = CollectionFormatApiClientBuilder::default()
        .base_url(mock_server.uri())
        .build()
        .expect("build client");

    let response = client
        .search_csv(vec![
            "rust".to_string(),
            "http".to_string(),
            "async".to_string(),
        ])
        .await
        .expect("search");
    assert_eq!(response.query, "csv");
}

#[tokio::test]
async fn test_collection_format_ssv() {
    let mock_server = MockServer::start().await;

    let result = SearchResult {
        query: "ssv".to_string(),
        count: 3,
    };

    // SSV format should produce a single param with space-separated values (URL encoded)
    Mock::given(method("GET"))
        .and(path("/search-ssv"))
        .and(query_param("tags", "rust http async"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&result))
        .mount(&mock_server)
        .await;

    let client = CollectionFormatApiClientBuilder::default()
        .base_url(mock_server.uri())
        .build()
        .expect("build client");

    let response = client
        .search_ssv(vec![
            "rust".to_string(),
            "http".to_string(),
            "async".to_string(),
        ])
        .await
        .expect("search");
    assert_eq!(response.query, "ssv");
}

#[tokio::test]
async fn test_collection_format_pipes() {
    let mock_server = MockServer::start().await;

    let result = SearchResult {
        query: "pipes".to_string(),
        count: 3,
    };

    // Pipes format should produce a single param with pipe-separated values
    Mock::given(method("GET"))
        .and(path("/search-pipes"))
        .and(query_param("tags", "rust|http|async"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&result))
        .mount(&mock_server)
        .await;

    let client = CollectionFormatApiClientBuilder::default()
        .base_url(mock_server.uri())
        .build()
        .expect("build client");

    let response = client
        .search_pipes(vec![
            "rust".to_string(),
            "http".to_string(),
            "async".to_string(),
        ])
        .await
        .expect("search");
    assert_eq!(response.query, "pipes");
}

#[tokio::test]
async fn test_collection_format_empty_vec() {
    let mock_server = MockServer::start().await;

    let result = SearchResult {
        query: "empty".to_string(),
        count: 0,
    };

    // Empty vec should not add any query param
    Mock::given(method("GET"))
        .and(path("/search-csv"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&result))
        .mount(&mock_server)
        .await;

    let client = CollectionFormatApiClientBuilder::default()
        .base_url(mock_server.uri())
        .build()
        .expect("build client");

    let response = client.search_csv(vec![]).await.expect("search");
    assert_eq!(response.query, "empty");
}

// ============================================================================
// Multipart Upload Tests
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
struct UploadResult {
    received_parts: usize,
    total_bytes: usize,
}

#[pincer(url = "http://localhost")]
trait MultipartApi {
    #[post("/upload")]
    async fn upload_file(&self, #[multipart] file: pincer::Part) -> pincer::Result<UploadResult>;

    #[post("/upload-named")]
    async fn upload_named(
        &self,
        #[multipart(name = "document")] file: pincer::Part,
    ) -> pincer::Result<UploadResult>;
}

#[tokio::test]
async fn test_multipart_upload_single_file() {
    let mock_server = MockServer::start().await;

    let result = UploadResult {
        received_parts: 1,
        total_bytes: 11,
    };

    Mock::given(method("POST"))
        .and(path("/upload"))
        .and(wiremock::matchers::header_exists("content-type"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&result))
        .mount(&mock_server)
        .await;

    let client = MultipartApiClientBuilder::default()
        .base_url(mock_server.uri())
        .build()
        .expect("build client");

    let part = pincer::Part::file("upload", "test.txt", "hello world");
    let response = client.upload_file(part).await.expect("upload");
    assert_eq!(response.received_parts, 1);
    assert_eq!(response.total_bytes, 11);
}

#[tokio::test]
async fn test_multipart_upload_with_custom_name() {
    let mock_server = MockServer::start().await;

    let result = UploadResult {
        received_parts: 1,
        total_bytes: 5,
    };

    Mock::given(method("POST"))
        .and(path("/upload-named"))
        .and(wiremock::matchers::header_exists("content-type"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&result))
        .mount(&mock_server)
        .await;

    let client = MultipartApiClientBuilder::default()
        .base_url(mock_server.uri())
        .build()
        .expect("build client");

    let part = pincer::Part::file("doc", "report.pdf", "data!");
    let response = client.upload_named(part).await.expect("upload");
    assert_eq!(response.received_parts, 1);
    assert_eq!(response.total_bytes, 5);
}

// ============================================================================
// Auto Path Detection Tests (Feature 1)
// ============================================================================

#[pincer(url = "http://localhost")]
trait AutoPathApi {
    /// Path param `id` auto-detected from URL placeholder `{id}`
    #[get("/users/{id}")]
    async fn get_user(&self, id: u64) -> pincer::Result<User>;

    /// Multiple path params auto-detected
    #[get("/repos/{owner}/{repo}")]
    async fn get_repo(&self, owner: &str, repo: &str) -> pincer::Result<SearchResult>;

    /// Explicit path alias when param name differs from placeholder
    #[get("/users/{user_id}")]
    async fn get_user_aliased(&self, #[path("user_id")] id: u64) -> pincer::Result<User>;
}

#[tokio::test]
async fn test_auto_path_detection() {
    let mock_server = MockServer::start().await;

    let user = User {
        id: 42,
        name: "Alice".to_string(),
    };

    Mock::given(method("GET"))
        .and(path("/users/42"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&user))
        .mount(&mock_server)
        .await;

    let client = AutoPathApiClientBuilder::default()
        .base_url(mock_server.uri())
        .build()
        .expect("build client");

    let response = client.get_user(42).await.expect("get user");
    assert_eq!(response.id, 42);
    assert_eq!(response.name, "Alice");
}

#[tokio::test]
async fn test_auto_path_detection_multiple() {
    let mock_server = MockServer::start().await;

    let result = SearchResult {
        query: "repo".to_string(),
        count: 100,
    };

    Mock::given(method("GET"))
        .and(path("/repos/rust-lang/rust"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&result))
        .mount(&mock_server)
        .await;

    let client = AutoPathApiClientBuilder::default()
        .base_url(mock_server.uri())
        .build()
        .expect("build client");

    let response = client
        .get_repo("rust-lang", "rust")
        .await
        .expect("get repo");
    assert_eq!(response.query, "repo");
}

#[tokio::test]
async fn test_auto_path_detection_with_alias() {
    let mock_server = MockServer::start().await;

    let user = User {
        id: 123,
        name: "Bob".to_string(),
    };

    Mock::given(method("GET"))
        .and(path("/users/123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&user))
        .mount(&mock_server)
        .await;

    let client = AutoPathApiClientBuilder::default()
        .base_url(mock_server.uri())
        .build()
        .expect("build client");

    let response = client.get_user_aliased(123).await.expect("get user");
    assert_eq!(response.id, 123);
}

// ============================================================================
// Auto Body Detection Tests (Feature 2)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CreateUser {
    name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UpdateUser {
    name: String,
}

#[pincer(url = "http://localhost")]
trait AutoBodyApi {
    /// Body param auto-detected (no explicit #[body])
    #[post("/users")]
    async fn create_user(&self, user: &CreateUser) -> pincer::Result<User>;

    /// Path auto-detected, body auto-detected
    #[put("/users/{id}")]
    async fn update_user(&self, id: u64, user: &UpdateUser) -> pincer::Result<User>;
}

#[tokio::test]
async fn test_auto_body_detection() {
    let mock_server = MockServer::start().await;

    let created_user = User {
        id: 1,
        name: "Charlie".to_string(),
    };

    Mock::given(method("POST"))
        .and(path("/users"))
        .and(header("content-type", "application/json"))
        .respond_with(ResponseTemplate::new(201).set_body_json(&created_user))
        .mount(&mock_server)
        .await;

    let client = AutoBodyApiClientBuilder::default()
        .base_url(mock_server.uri())
        .build()
        .expect("build client");

    let new_user = CreateUser {
        name: "Charlie".to_string(),
    };
    let response = client.create_user(&new_user).await.expect("create user");
    assert_eq!(response.id, 1);
    assert_eq!(response.name, "Charlie");
}

#[tokio::test]
async fn test_auto_body_with_auto_path() {
    let mock_server = MockServer::start().await;

    let updated_user = User {
        id: 42,
        name: "Updated".to_string(),
    };

    Mock::given(method("PUT"))
        .and(path("/users/42"))
        .and(header("content-type", "application/json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&updated_user))
        .mount(&mock_server)
        .await;

    let client = AutoBodyApiClientBuilder::default()
        .base_url(mock_server.uri())
        .build()
        .expect("build client");

    let update = UpdateUser {
        name: "Updated".to_string(),
    };
    let response = client.update_user(42, &update).await.expect("update user");
    assert_eq!(response.id, 42);
    assert_eq!(response.name, "Updated");
}

// ============================================================================
// Query Derive Macro Tests (Feature 3)
// ============================================================================

#[derive(Debug, Clone, Query)]
struct QueryDeriveTest {
    q: String,
    page: Option<u32>,
    #[query(rename = "per_page")]
    limit: u32,
    #[query(format = "csv")]
    tags: Vec<String>,
}

#[test]
fn test_query_derive_basic() {
    let params = QueryDeriveTest {
        q: "rust".to_string(),
        page: Some(2),
        limit: 10,
        tags: vec!["a".to_string(), "b".to_string()],
    };

    let pairs = params.to_query_pairs();

    assert!(pairs.contains(&("q".to_string(), "rust".to_string())));
    assert!(pairs.contains(&("page".to_string(), "2".to_string())));
    assert!(pairs.contains(&("per_page".to_string(), "10".to_string())));
    assert!(pairs.contains(&("tags".to_string(), "a,b".to_string())));
}

#[test]
fn test_query_derive_skip_none() {
    let params = QueryDeriveTest {
        q: "rust".to_string(),
        page: None, // should be skipped
        limit: 10,
        tags: vec![],
    };

    let pairs = params.to_query_pairs();

    assert!(pairs.contains(&("q".to_string(), "rust".to_string())));
    assert!(!pairs.iter().any(|(k, _)| k == "page"));
    assert!(pairs.contains(&("per_page".to_string(), "10".to_string())));
    // empty vec doesn't add anything with csv format
    assert!(!pairs.iter().any(|(k, _)| k == "tags"));
}

#[derive(Debug, Clone, Query)]
#[allow(clippy::struct_field_names)]
struct QueryDeriveMultiFormat {
    #[query(format = "multi")]
    multi_tags: Vec<String>,
    #[query(format = "ssv")]
    ssv_tags: Vec<String>,
    #[query(format = "pipes")]
    pipe_tags: Vec<String>,
}

#[test]
fn test_query_derive_collection_formats() {
    let params = QueryDeriveMultiFormat {
        multi_tags: vec!["a".to_string(), "b".to_string()],
        ssv_tags: vec!["c".to_string(), "d".to_string()],
        pipe_tags: vec!["e".to_string(), "f".to_string()],
    };

    let pairs = params.to_query_pairs();

    // Multi format produces multiple pairs
    assert!(pairs.contains(&("multi_tags".to_string(), "a".to_string())));
    assert!(pairs.contains(&("multi_tags".to_string(), "b".to_string())));

    // SSV format produces space-separated value
    assert!(pairs.contains(&("ssv_tags".to_string(), "c d".to_string())));

    // Pipes format produces pipe-separated value
    assert!(pairs.contains(&("pipe_tags".to_string(), "e|f".to_string())));
}

// Test rename_all with camelCase
#[derive(Debug, Clone, Query)]
#[query(rename_all = "camelCase")]
struct QueryDeriveCamelCase {
    search_query: String,
    page_number: u32,
    #[query(rename = "limit")] // explicit rename overrides rename_all
    per_page: u32,
}

#[test]
fn test_query_derive_rename_all_camel_case() {
    let params = QueryDeriveCamelCase {
        search_query: "rust".to_string(),
        page_number: 1,
        per_page: 10,
    };

    let pairs = params.to_query_pairs();

    // snake_case fields are converted to camelCase
    assert!(pairs.contains(&("searchQuery".to_string(), "rust".to_string())));
    assert!(pairs.contains(&("pageNumber".to_string(), "1".to_string())));
    // explicit rename overrides rename_all
    assert!(pairs.contains(&("limit".to_string(), "10".to_string())));
}

// Test rename_all with kebab-case
#[derive(Debug, Clone, Query)]
#[query(rename_all = "kebab-case")]
struct QueryDeriveKebabCase {
    search_query: String,
    page_number: u32,
}

#[test]
fn test_query_derive_rename_all_kebab_case() {
    let params = QueryDeriveKebabCase {
        search_query: "rust".to_string(),
        page_number: 1,
    };

    let pairs = params.to_query_pairs();

    // snake_case fields are converted to kebab-case
    assert!(pairs.contains(&("search-query".to_string(), "rust".to_string())));
    assert!(pairs.contains(&("page-number".to_string(), "1".to_string())));
}

// Test rename_all with SCREAMING_SNAKE_CASE
#[derive(Debug, Clone, Query)]
#[query(rename_all = "SCREAMING_SNAKE_CASE")]
struct QueryDeriveScreamingSnake {
    search_query: String,
    page_number: u32,
}

#[test]
fn test_query_derive_rename_all_screaming_snake() {
    let params = QueryDeriveScreamingSnake {
        search_query: "rust".to_string(),
        page_number: 1,
    };

    let pairs = params.to_query_pairs();

    // snake_case fields are converted to SCREAMING_SNAKE_CASE
    assert!(pairs.contains(&("SEARCH_QUERY".to_string(), "rust".to_string())));
    assert!(pairs.contains(&("PAGE_NUMBER".to_string(), "1".to_string())));
}

// ============================================================================
// Wrapper Mode Tests (mode = "wrapper")
// ============================================================================

/// Test wrapper mode: generates a generic wrapper struct that works with any `PincerClient`
#[pincer(url = "http://localhost:9999", mode = "wrapper")]
pub trait WrapperModeApi {
    #[get("/users/{id}")]
    async fn get_user(&self, #[path] id: u64) -> pincer::Result<User>;

    #[get("/health")]
    async fn health(&self) -> pincer::Result<()>;
}

#[tokio::test]
async fn test_wrapper_mode_with_api_client() {
    let mock_server = MockServer::start().await;

    let user = User {
        id: 42,
        name: "Alice".to_string(),
    };

    Mock::given(method("GET"))
        .and(path("/users/42"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&user))
        .mount(&mock_server)
        .await;

    // Create a HyperClient and wrap it with ApiClient
    let http = pincer::HyperClient::new();
    let api = pincer::ApiClient::new(http, mock_server.uri()).expect("api client");

    // Use the wrapper struct with the ApiClient and mock server URL
    let base_url = pincer::url::Url::parse(&mock_server.uri()).expect("parse url");
    let client = WrapperModeApiClient::with_base_url(api, base_url);

    // Use explicit trait method call to disambiguate from ImplOnlyApi blanket impl
    let result = WrapperModeApi::get_user(&client, 42)
        .await
        .expect("get user");
    assert_eq!(result.id, 42);
    assert_eq!(result.name, "Alice");
}

#[tokio::test]
async fn test_wrapper_mode_with_custom_base_url() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/health"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock_server)
        .await;

    // Create ApiClient and wrap with WrapperModeApiClient using custom base URL
    let http = pincer::HyperClient::new();
    let api = pincer::ApiClient::new(http, mock_server.uri()).expect("api client");
    let base_url = pincer::url::Url::parse(&mock_server.uri()).expect("parse url");

    let client = WrapperModeApiClient::with_base_url(api, base_url);

    client.health().await.expect("health check");
}

#[test]
fn test_wrapper_mode_struct_is_clone() {
    // Verify the wrapper struct implements Clone when the inner client is Clone
    let http = pincer::HyperClient::new();
    let api = pincer::ApiClient::new(http, "http://localhost").expect("api client");
    let client = WrapperModeApiClient::new(api);
    let _cloned = client.clone();
}

// ============================================================================
// Impl-Only Mode Tests (mode = "impl_only")
// ============================================================================

/// Test `impl_only` mode: generates a blanket impl for any `PincerClient`
#[pincer(mode = "impl_only")]
pub trait ImplOnlyApi {
    #[get("/users/{id}")]
    async fn get_user(&self, #[path] id: u64) -> pincer::Result<User>;

    #[post("/users")]
    async fn create_user(&self, user: &CreateUser) -> pincer::Result<User>;
}

#[tokio::test]
async fn test_impl_only_mode_with_api_client() {
    let mock_server = MockServer::start().await;

    let user = User {
        id: 99,
        name: "Bob".to_string(),
    };

    Mock::given(method("GET"))
        .and(path("/users/99"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&user))
        .mount(&mock_server)
        .await;

    // Create ApiClient - it directly implements ImplOnlyApi via blanket impl
    let http = pincer::HyperClient::new();
    let api = pincer::ApiClient::new(http, mock_server.uri()).expect("api client");

    // ApiClient directly implements ImplOnlyApi
    let result = api.get_user(99).await.expect("get user");
    assert_eq!(result.id, 99);
    assert_eq!(result.name, "Bob");
}

#[tokio::test]
async fn test_impl_only_mode_post_request() {
    let mock_server = MockServer::start().await;

    let created_user = User {
        id: 1,
        name: "Charlie".to_string(),
    };

    Mock::given(method("POST"))
        .and(path("/users"))
        .and(header("content-type", "application/json"))
        .respond_with(ResponseTemplate::new(201).set_body_json(&created_user))
        .mount(&mock_server)
        .await;

    let http = pincer::HyperClient::new();
    let api = pincer::ApiClient::new(http, mock_server.uri()).expect("api client");

    let new_user = CreateUser {
        name: "Charlie".to_string(),
    };
    let result = api.create_user(&new_user).await.expect("create user");
    assert_eq!(result.id, 1);
    assert_eq!(result.name, "Charlie");
}

// ============================================================================
// Custom PincerClient Implementation Test
// ============================================================================

/// A custom `PincerClient` that adds headers to every request
#[derive(Clone)]
struct AuthenticatedClient {
    inner: pincer::HyperClient,
    base_url: pincer::url::Url,
    api_key: String,
}

impl AuthenticatedClient {
    fn new(base_url: &str, api_key: &str) -> pincer::Result<Self> {
        Ok(Self {
            inner: pincer::HyperClient::new(),
            base_url: pincer::url::Url::parse(base_url).map_err(pincer::Error::InvalidUrl)?,
            api_key: api_key.to_string(),
        })
    }
}

impl pincer::PincerClient for AuthenticatedClient {
    fn execute(
        &self,
        request: pincer::Request<bytes::Bytes>,
    ) -> impl std::future::Future<Output = pincer::Result<pincer::Response<bytes::Bytes>>> + Send
    {
        // Add API key header to every request
        let api_key = self.api_key.clone();
        let inner = self.inner.clone();
        async move {
            let mut req = request;
            req.headers_mut().insert("X-API-Key".to_string(), api_key);
            inner.execute(req).await
        }
    }

    fn base_url(&self) -> &pincer::url::Url {
        &self.base_url
    }
}

#[tokio::test]
async fn test_custom_pincer_client_with_impl_only() {
    let mock_server = MockServer::start().await;

    let user = User {
        id: 1,
        name: "Test".to_string(),
    };

    // Verify that the custom X-API-Key header is sent
    Mock::given(method("GET"))
        .and(path("/users/1"))
        .and(header("X-API-Key", "secret-key-123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&user))
        .mount(&mock_server)
        .await;

    let client =
        AuthenticatedClient::new(&mock_server.uri(), "secret-key-123").expect("auth client");

    // Custom client implements ImplOnlyApi via blanket impl
    let result = client.get_user(1).await.expect("get user");
    assert_eq!(result.id, 1);
}

#[tokio::test]
async fn test_custom_pincer_client_with_wrapper_mode() {
    let mock_server = MockServer::start().await;

    let user = User {
        id: 2,
        name: "Wrapped".to_string(),
    };

    Mock::given(method("GET"))
        .and(path("/users/2"))
        .and(header("X-API-Key", "my-api-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&user))
        .mount(&mock_server)
        .await;

    let auth_client =
        AuthenticatedClient::new(&mock_server.uri(), "my-api-key").expect("auth client");

    // Wrap the custom client with WrapperModeApiClient using mock server URL
    let base_url = pincer::url::Url::parse(&mock_server.uri()).expect("parse url");
    let client = WrapperModeApiClient::with_base_url(auth_client, base_url);

    // Use explicit trait method call to disambiguate
    let result = WrapperModeApi::get_user(&client, 2)
        .await
        .expect("get user");
    assert_eq!(result.id, 2);
}
