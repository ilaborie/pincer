//! Wikipedia API Example
//!
//! Demonstrates pincer's declarative HTTP client pattern with lazy pagination.

// Example-specific lint allowances
#![allow(missing_docs)]
#![allow(clippy::unused_async)]
#![allow(clippy::print_stdout)]
#![allow(dead_code)]

use pincer::prelude::*;

// ============================================================================
// Data Types
// ============================================================================

/// A Wikipedia page from search results.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Page {
    #[serde(rename = "pageid")]
    pub id: u64,
    pub title: String,
}

/// Continue token for pagination.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Continue {
    #[serde(rename = "gsroffset")]
    pub offset: Option<u64>,
    #[serde(rename = "continue")]
    pub continue_token: Option<String>,
}

/// Query result containing pages.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QueryResult {
    #[serde(default)]
    pub pages: std::collections::HashMap<String, Page>,
}

/// Wikipedia API response wrapper.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WikiResponse {
    #[serde(default)]
    pub query: Option<QueryResult>,
    #[serde(rename = "continue")]
    pub continue_token: Option<Continue>,
}

impl WikiResponse {
    /// Get the next offset for pagination, if available.
    #[must_use]
    pub fn next_offset(&self) -> Option<u64> {
        self.continue_token.as_ref()?.offset
    }

    /// Get pages as a sorted vector (by title).
    #[must_use]
    pub fn pages(&self) -> Vec<Page> {
        let mut pages: Vec<Page> = self
            .query
            .as_ref()
            .map(|q| q.pages.values().cloned().collect())
            .unwrap_or_default();
        pages.sort_by(|a, b| a.title.cmp(&b.title));
        pages
    }
}

// ============================================================================
// Declarative API using pincer proc-macros (trait-based)
// ============================================================================

/// Wikipedia API client using declarative proc-macro syntax.
#[pincer(
    url = "https://en.wikipedia.org",
    user_agent = "pincer-wikipedia-example/0.1.0"
)]
pub trait WikipediaApi {
    /// Search Wikipedia pages.
    #[get("/w/api.php")]
    async fn search(
        &self,
        #[query] action: &str,
        #[query] generator: &str,
        #[query] prop: &str,
        #[query] format: &str,
        #[query] gsrsearch: &str,
    ) -> pincer::Result<WikiResponse>;

    /// Resume search with pagination offset.
    #[get("/w/api.php")]
    async fn resume_search(
        &self,
        #[query] action: &str,
        #[query] generator: &str,
        #[query] prop: &str,
        #[query] format: &str,
        #[query] gsrsearch: &str,
        #[query] gsroffset: u64,
    ) -> pincer::Result<WikiResponse>;
}

// Helper methods to simplify the API
impl WikipediaApiClient {
    /// Create a new builder with default settings.
    #[must_use]
    pub fn default_builder() -> WikipediaApiClientBuilder {
        WikipediaApiClientBuilder::default()
    }

    /// Simplified search with common parameters.
    pub async fn simple_search(&self, query: &str) -> pincer::Result<WikiResponse> {
        self.search("query", "search", "info", "json", query).await
    }

    /// Simplified resume search with common parameters.
    pub async fn simple_resume_search(
        &self,
        query: &str,
        offset: u64,
    ) -> pincer::Result<WikiResponse> {
        self.resume_search("query", "search", "info", "json", query, offset)
            .await
    }
}

// ============================================================================
// Lazy Search Iterator
// ============================================================================

/// A lazy iterator that fetches more pages as needed.
pub struct LazySearch<'a> {
    client: &'a WikipediaApiClient,
    query: String,
    current_pages: std::vec::IntoIter<Page>,
    next_offset: Option<u64>,
}

impl<'a> LazySearch<'a> {
    /// Create a new lazy search iterator.
    pub async fn new(
        client: &'a WikipediaApiClient,
        query: impl Into<String>,
    ) -> pincer::Result<Self> {
        let query = query.into();
        let response = client.simple_search(&query).await?;
        let next_offset = response.next_offset();
        let current_pages = response.pages().into_iter();

        Ok(Self {
            client,
            query,
            current_pages,
            next_offset,
        })
    }

    /// Get the next page, fetching more results if needed.
    ///
    /// Note: This is async because it may need to make HTTP requests.
    pub async fn next_page(&mut self) -> pincer::Result<Option<Page>> {
        // Try to get next from current batch
        if let Some(page) = self.current_pages.next() {
            return Ok(Some(page));
        }

        // If we have more pages to fetch, get them
        if let Some(offset) = self.next_offset {
            println!("Wow.. even more results than {offset}");
            let response = self
                .client
                .simple_resume_search(&self.query, offset)
                .await?;
            self.next_offset = response.next_offset();
            self.current_pages = response.pages().into_iter();

            return Ok(self.current_pages.next());
        }

        Ok(None)
    }
}

// ============================================================================
// Main: Demonstrate usage
// ============================================================================

#[tokio::main]
async fn main() -> pincer::Result<()> {
    // Create client using the generated builder
    let wikipedia = WikipediaApiClient::default_builder()
        .base_url("https://en.wikipedia.org")
        .build()?;

    println!("Wikipedia API Client created!");
    println!("Base URL: {}", wikipedia.base_url());

    // Note: These calls would work with the real Wikipedia API
    // For demonstration, we just show the API structure

    println!("\n=== Example API calls (would require real API) ===");
    println!("wikipedia.simple_search(\"Rust programming\").await?");
    println!("wikipedia.simple_resume_search(\"Rust programming\", 10).await?");

    Ok(())
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

    fn mock_response(pages: Vec<Page>, next_offset: Option<u64>) -> WikiResponse {
        let mut page_map = std::collections::HashMap::new();
        for page in pages {
            page_map.insert(page.id.to_string(), page);
        }

        WikiResponse {
            query: Some(QueryResult { pages: page_map }),
            continue_token: next_offset.map(|o| Continue {
                offset: Some(o),
                continue_token: Some("-||".to_string()),
            }),
        }
    }

    #[tokio::test]
    async fn test_search() {
        let mock_server = MockServer::start().await;

        let pages = vec![
            Page {
                id: 1001,
                title: "test Result #1".to_string(),
            },
            Page {
                id: 1002,
                title: "test Result #2".to_string(),
            },
        ];

        Mock::given(method("GET"))
            .and(path("/w/api.php"))
            .and(query_param("action", "query"))
            .and(query_param("generator", "search"))
            .and(query_param("gsrsearch", "test"))
            .and(header("Accept", "application/json"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(mock_response(pages.clone(), None)),
            )
            .mount(&mock_server)
            .await;

        let wikipedia = WikipediaApiClient::default_builder()
            .base_url(mock_server.uri())
            .build()
            .expect("client");

        let response = wikipedia.simple_search("test").await.expect("search");

        let result_pages = response.pages();
        assert_eq!(result_pages.len(), 2);
    }

    #[tokio::test]
    async fn test_search_with_pagination() {
        let mock_server = MockServer::start().await;

        let pages1 = vec![Page {
            id: 1001,
            title: "Rust Result #1".to_string(),
        }];

        let pages2 = vec![Page {
            id: 1002,
            title: "Rust Result #2".to_string(),
        }];

        // Second page (with offset) - register first so it has higher priority
        Mock::given(method("GET"))
            .and(path("/w/api.php"))
            .and(query_param("gsrsearch", "Rust"))
            .and(query_param("gsroffset", "10"))
            .respond_with(ResponseTemplate::new(200).set_body_json(mock_response(pages2, None)))
            .mount(&mock_server)
            .await;

        // First page (without offset)
        Mock::given(method("GET"))
            .and(path("/w/api.php"))
            .and(query_param("gsrsearch", "Rust"))
            .respond_with(ResponseTemplate::new(200).set_body_json(mock_response(pages1, Some(10))))
            .mount(&mock_server)
            .await;

        let wikipedia = WikipediaApiClient::default_builder()
            .base_url(mock_server.uri())
            .build()
            .expect("client");

        // First page
        let response = wikipedia.simple_search("Rust").await.expect("search");
        assert!(!response.pages().is_empty());
        assert_eq!(response.next_offset(), Some(10));

        // Second page
        let response2 = wikipedia
            .simple_resume_search("Rust", 10)
            .await
            .expect("resume");
        assert!(!response2.pages().is_empty());
        assert!(response2.next_offset().is_none());
    }

    #[tokio::test]
    async fn test_lazy_search() {
        let mock_server = MockServer::start().await;

        let pages1: Vec<Page> = (1..=10)
            .map(|i| Page {
                id: 1000 + i,
                title: format!("test Result #{i}"),
            })
            .collect();

        let pages2: Vec<Page> = (11..=20)
            .map(|i| Page {
                id: 1000 + i,
                title: format!("test Result #{i}"),
            })
            .collect();

        let pages3: Vec<Page> = (21..=25)
            .map(|i| Page {
                id: 1000 + i,
                title: format!("test Result #{i}"),
            })
            .collect();

        // Register mocks from most specific to least specific
        // Third page (most specific - has gsroffset=20)
        Mock::given(method("GET"))
            .and(path("/w/api.php"))
            .and(query_param("gsrsearch", "test"))
            .and(query_param("gsroffset", "20"))
            .respond_with(ResponseTemplate::new(200).set_body_json(mock_response(pages3, None)))
            .mount(&mock_server)
            .await;

        // Second page (has gsroffset=10)
        Mock::given(method("GET"))
            .and(path("/w/api.php"))
            .and(query_param("gsrsearch", "test"))
            .and(query_param("gsroffset", "10"))
            .respond_with(ResponseTemplate::new(200).set_body_json(mock_response(pages2, Some(20))))
            .mount(&mock_server)
            .await;

        // First page (least specific - no gsroffset)
        Mock::given(method("GET"))
            .and(path("/w/api.php"))
            .and(query_param("gsrsearch", "test"))
            .respond_with(ResponseTemplate::new(200).set_body_json(mock_response(pages1, Some(10))))
            .mount(&mock_server)
            .await;

        let wikipedia = WikipediaApiClient::default_builder()
            .base_url(mock_server.uri())
            .build()
            .expect("client");

        let mut search = LazySearch::new(&wikipedia, "test")
            .await
            .expect("lazy search");

        // Collect all results
        let mut pages = Vec::new();
        while let Some(page) = search.next_page().await.expect("next page") {
            pages.push(page);
        }

        // We should have 25 results (across 3 pages)
        assert_eq!(pages.len(), 25);
    }
}
