//! # Chapter 0: Getting Started
//!
//! Your first pincer API client in 5 minutes.
//!
//! ## What You'll Learn
//!
//! - Define an API trait with `#[pincer]`
//! - Make GET requests with `#[get]`
//! - Use the generated builder and client
//!
//! ## Prerequisites
//!
//! Add to `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! pincer = "0.1"
//! serde = { version = "1.0", features = ["derive"] }
//! tokio = { version = "1", features = ["full"] }
//! ```
//!
//! ## Your First Client
//!
//! ```ignore
//! use pincer::prelude::*;
//!
//! // Define response types
//! #[derive(Debug, Deserialize)]
//! pub struct User {
//!     pub id: u64,
//!     pub name: String,
//! }
//!
//! // Define the API as a trait
//! #[pincer(url = "https://api.example.com")]
//! pub trait UserApi {
//!     #[get("/users/{id}")]
//!     async fn get_user(&self, #[path] id: u64) -> pincer::Result<User>;
//! }
//!
//! // Use it
//! #[tokio::main]
//! async fn main() -> pincer::Result<()> {
//!     let client = UserApiClientBuilder::default().build()?;
//!     let user = client.get_user(42).await?;
//!     println!("User: {:?}", user);
//!     Ok(())
//! }
//! ```
//!
//! ## What Gets Generated
//!
//! The `#[pincer]` macro generates:
//!
//! ```text
//! #[pincer(url = "...")]        UserApiClient         (struct)
//! pub trait UserApi { ... }  →  UserApiClientBuilder  (builder)
//!                               impl UserApi for ...  (methods)
//! ```
//!
//! - `UserApiClient` - The client struct holding the HTTP client and base URL
//! - `UserApiClientBuilder` - Builder for configuring the client
//! - `impl UserApi` - Implementation of your trait methods
//!
//! ## Next Steps
//!
//! - [Chapter 1: Parameters & Bodies][super::chapter_1] - Path, query, headers, JSON bodies
