//! GitHub API Example
//!
//! Demonstrates pincer's declarative HTTP client pattern.

// Example-specific lint allowances
#![allow(missing_docs)]
#![allow(clippy::unused_async)]
#![allow(clippy::print_stdout)]
#![allow(dead_code)]

use pincer::prelude::*;

// ============================================================================
// Data Types
// ============================================================================

/// A GitHub contributor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Contributor {
    pub login: String,
    pub contributions: u32,
}

/// A GitHub repository.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Repository {
    pub id: u64,
    pub name: String,
    pub full_name: String,
    pub description: Option<String>,
    pub stargazers_count: u32,
    pub forks_count: u32,
}

/// Request to create a GitHub issue.
#[derive(Debug, Clone, Serialize)]
pub struct CreateIssue {
    pub title: String,
    pub body: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub assignees: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,
}

/// A GitHub issue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Issue {
    pub id: u64,
    pub number: u32,
    pub title: String,
    pub body: Option<String>,
    pub state: String,
}

// ============================================================================
// Declarative API using pincer proc-macros (trait-based)
// ============================================================================

/// GitHub API client using declarative proc-macro syntax.
#[pincer(
    url = "https://api.github.com",
    user_agent = "pincer-github-example/0.1.0"
)]
pub trait GitHubApi {
    /// List contributors for a repository.
    #[get("/repos/{owner}/{repo}/contributors")]
    async fn contributors(
        &self,
        #[path] owner: &str,
        #[path] repo: &str,
    ) -> pincer::Result<Vec<Contributor>>;

    /// Get repository information.
    #[get("/repos/{owner}/{repo}")]
    async fn get_repo(&self, #[path] owner: &str, #[path] repo: &str)
    -> pincer::Result<Repository>;

    /// Create an issue.
    #[post("/repos/{owner}/{repo}/issues")]
    async fn create_issue(
        &self,
        #[path] owner: &str,
        #[path] repo: &str,
        #[body] issue: &CreateIssue,
        #[header("Authorization")] token: &str,
    ) -> pincer::Result<Issue>;

    /// List issues with optional query parameters.
    #[get("/repos/{owner}/{repo}/issues")]
    async fn list_issues(
        &self,
        #[path] owner: &str,
        #[path] repo: &str,
        #[query] state: Option<&str>,
        #[query] per_page: Option<u32>,
        #[query] page: Option<u32>,
    ) -> pincer::Result<Vec<Issue>>;

    /// Example using the #[http] attribute for custom HTTP methods.
    #[http("OPTIONS /repos/{owner}/{repo}")]
    async fn repo_options(&self, #[path] owner: &str, #[path] repo: &str) -> pincer::Result<()>;
}

// ============================================================================
// Main: Demonstrate usage
// ============================================================================

#[tokio::main]
async fn main() -> pincer::Result<()> {
    // Create client using the trait's client() factory
    let github = GitHubApiClient::default_builder()
        .base_url("https://api.github.com")
        .build()?;

    println!("GitHub API Client created!");
    println!("Base URL: {}", github.base_url());

    // Create client with middleware using configure_client
    // This demonstrates the Tower middleware integration
    let github_with_middleware = GitHubApiClient::default_builder()
        .base_url("https://api.github.com")
        .configure_client(|builder| {
            builder
                .with_retry(3) // Retry on 5xx/429/connection errors
                .with_logging() // Log requests and responses
        })
        .build()?;

    println!("\nGitHub API Client with middleware created!");
    println!("Base URL: {}", github_with_middleware.base_url());

    // Note: These calls would work with a real GitHub API token
    // For demonstration, we just show the API structure

    println!("\n=== Example API calls (would require real API) ===");
    println!("github.contributors(\"rust-lang\", \"rust\").await?");
    println!("github.get_repo(\"rust-lang\", \"rust\").await?");
    println!(
        "github.list_issues(\"rust-lang\", \"rust\", Some(\"open\"), Some(10), Some(1)).await?"
    );

    Ok(())
}

// Helper impl for ergonomic builder access
impl GitHubApiClient {
    /// Create a new builder with default settings.
    #[must_use]
    pub fn default_builder() -> GitHubApiClientBuilder {
        GitHubApiClientBuilder::default()
    }
}

// ============================================================================
// Tests using wiremock
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{header, method, path, query_param},
    };

    #[tokio::test]
    async fn test_contributors() {
        let mock_server = MockServer::start().await;

        let contributors = vec![
            Contributor {
                login: "user1".to_string(),
                contributions: 100,
            },
            Contributor {
                login: "user2".to_string(),
                contributions: 50,
            },
        ];

        Mock::given(method("GET"))
            .and(path("/repos/rust-lang/rust/contributors"))
            .and(header("Accept", "application/json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&contributors))
            .mount(&mock_server)
            .await;

        let github = GitHubApiClient::default_builder()
            .base_url(mock_server.uri())
            .build()
            .expect("client");

        let result = github
            .contributors("rust-lang", "rust")
            .await
            .expect("contributors");

        assert_eq!(result.len(), 2);
        let first = result.first().expect("first contributor");
        assert_eq!(first.login, "user1");
        assert_eq!(first.contributions, 100);
    }

    #[tokio::test]
    async fn test_get_repo() {
        let mock_server = MockServer::start().await;

        let repo = Repository {
            id: 12345,
            name: "rust".to_string(),
            full_name: "rust-lang/rust".to_string(),
            description: Some("The Rust programming language".to_string()),
            stargazers_count: 90000,
            forks_count: 12000,
        };

        Mock::given(method("GET"))
            .and(path("/repos/rust-lang/rust"))
            .and(header("Accept", "application/json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&repo))
            .mount(&mock_server)
            .await;

        let github = GitHubApiClient::default_builder()
            .base_url(mock_server.uri())
            .build()
            .expect("client");

        let result = github.get_repo("rust-lang", "rust").await.expect("repo");

        assert_eq!(result.name, "rust");
        assert_eq!(result.full_name, "rust-lang/rust");
    }

    #[tokio::test]
    async fn test_list_issues_with_query_params() {
        let mock_server = MockServer::start().await;

        let issues = vec![Issue {
            id: 1,
            number: 42,
            title: "Example issue".to_string(),
            body: Some("Issue body".to_string()),
            state: "open".to_string(),
        }];

        Mock::given(method("GET"))
            .and(path("/repos/rust-lang/rust/issues"))
            .and(query_param("state", "open"))
            .and(query_param("per_page", "5"))
            .and(header("Accept", "application/json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&issues))
            .mount(&mock_server)
            .await;

        let github = GitHubApiClient::default_builder()
            .base_url(mock_server.uri())
            .build()
            .expect("client");

        let result = github
            .list_issues("rust-lang", "rust", Some("open"), Some(5), None)
            .await
            .expect("issues");

        assert!(!result.is_empty());
        assert_eq!(result.first().expect("first issue").number, 42);
    }
}
