//! # Chapter 1: Parameters & Bodies
//!
//! How to pass data to your API calls.
//!
//! ## Path Parameters
//!
//! Use `{name}` placeholders in the URL and `#[path]` on parameters:
//!
//! ```ignore
//! #[get("/repos/{owner}/{repo}")]
//! async fn get_repo(
//!     &self,
//!     #[path] owner: &str,
//!     #[path] repo: &str,
//! ) -> pincer::Result<Repo>;
//! ```
//!
//! ## Query Parameters
//!
//! Use `#[query]` for URL query strings (`?key=value`):
//!
//! ```ignore
//! #[get("/users")]
//! async fn list_users(
//!     &self,
//!     #[query] page: u32,
//!     #[query] per_page: u32,
//! ) -> pincer::Result<Vec<User>>;
//!
//! // Generates: GET /users?page=1&per_page=10
//! ```
//!
//! ### Query Structs with `#[derive(Query)]`
//!
//! For complex queries, derive `Query` on a struct:
//!
//! ```ignore
//! #[derive(Query)]
//! #[query(rename_all = "camelCase")]
//! pub struct SearchParams {
//!     pub search_term: String,
//!     #[query(skip_none)]
//!     pub page: Option<u32>,
//!     #[query(format = "csv")]
//!     pub tags: Vec<String>,
//! }
//!
//! #[get("/search")]
//! async fn search(&self, #[query] params: SearchParams) -> pincer::Result<Results>;
//!
//! // Generates: GET /search?searchTerm=rust&tags=async,http
//! ```
//!
//! ## Headers
//!
//! Single header with `#[header("Name")]`:
//!
//! ```ignore
//! #[get("/me")]
//! async fn get_me(
//!     &self,
//!     #[header("Authorization")] token: &str,
//! ) -> pincer::Result<User>;
//! ```
//!
//! Multiple headers with `#[headers]`:
//!
//! ```ignore
//! #[post("/webhook")]
//! async fn webhook(
//!     &self,
//!     #[headers] headers: HashMap<String, String>,
//! ) -> pincer::Result<()>;
//! ```
//!
//! ## Request Bodies
//!
//! ### JSON Body
//!
//! Use `#[body]` for JSON (default):
//!
//! ```ignore
//! #[post("/users")]
//! async fn create_user(
//!     &self,
//!     #[body] user: &CreateUser,
//! ) -> pincer::Result<User>;
//! ```
//!
//! ### Form Body
//!
//! Use `#[form]` for `application/x-www-form-urlencoded`:
//!
//! ```ignore
//! #[post("/login")]
//! async fn login(
//!     &self,
//!     #[form] credentials: &LoginForm,
//! ) -> pincer::Result<Token>;
//! ```
//!
//! ## Summary
//!
//! | Attribute | Purpose | Content-Type |
//! |-----------|---------|--------------|
//! | `#[path]` | URL path segment | - |
//! | `#[query]` | Query string | - |
//! | `#[header("X")]` | HTTP header | - |
//! | `#[body]` | JSON body | `application/json` |
//! | `#[form]` | Form body | `application/x-www-form-urlencoded` |
//!
//! ## Next Steps
//!
//! - [Chapter 2: Response Handling][super::chapter_2] - JSON deserialization, errors
