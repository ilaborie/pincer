//! # Chapter 2: Response Handling
//!
//! How to work with API responses and errors.
//!
//! ## Return Types
//!
//! ### JSON Response (Default)
//!
//! Return `pincer::Result<T>` where `T: Deserialize`:
//!
//! ```ignore
//! #[get("/users/{id}")]
//! async fn get_user(&self, #[path] id: u64) -> pincer::Result<User>;
//! ```
//!
//! ### Raw Response
//!
//! Return `Response<Bytes>` to access status, headers, and body:
//!
//! ```ignore
//! use pincer::Response;
//! use bytes::Bytes;
//!
//! #[get("/data")]
//! async fn get_data(&self) -> pincer::Result<Response<Bytes>>;
//!
//! // Usage:
//! let response = client.get_data().await?;
//! if response.is_success() {
//!     let body: MyData = response.json()?;
//! }
//! ```
//!
//! ### No Response Body
//!
//! Use `()` for endpoints that don't return data:
//!
//! ```ignore
//! #[delete("/users/{id}")]
//! async fn delete_user(&self, #[path] id: u64) -> pincer::Result<()>;
//! ```
//!
//! ## Error Handling
//!
//! ### Default Behavior
//!
//! By default, non-2xx responses return `Error::HttpStatus`:
//!
//! ```ignore
//! match client.get_user(404).await {
//!     Ok(user) => println!("Found: {:?}", user),
//!     Err(pincer::Error::HttpStatus { status, body }) => {
//!         eprintln!("HTTP {}: {}", status, String::from_utf8_lossy(&body));
//!     }
//!     Err(e) => eprintln!("Other error: {}", e),
//! }
//! ```
//!
//! ### Custom Error Decoder
//!
//! Implement [`ErrorDecoder`][crate::ErrorDecoder] to convert API errors:
//!
//! ```ignore
//! use pincer::{ErrorDecoder, Response, Error};
//! use bytes::Bytes;
//!
//! #[derive(Debug, Deserialize)]
//! struct ApiError {
//!     code: String,
//!     message: String,
//! }
//!
//! struct MyErrorDecoder;
//!
//! impl ErrorDecoder for MyErrorDecoder {
//!     fn decode(&self, response: &Response<Bytes>) -> Option<Error> {
//!         if response.is_client_error() || response.is_server_error() {
//!             if let Ok(api_error) = response.clone().json::<ApiError>() {
//!                 return Some(Error::custom(format!(
//!                     "[{}] {}",
//!                     api_error.code, api_error.message
//!                 )));
//!             }
//!         }
//!         None // Use default handling
//!     }
//! }
//!
//! // Use it:
//! let client = UserApiClientBuilder::default()
//!     .error_decoder(MyErrorDecoder)
//!     .build()?;
//! ```
//!
//! ## Status Code Helpers
//!
//! The [`Response`][crate::Response] type provides status helpers:
//!
//! ```text
//! response.is_success()      // 2xx
//! response.is_redirection()  // 3xx
//! response.is_client_error() // 4xx
//! response.is_server_error() // 5xx
//! ```
//!
//! ## Next Steps
//!
//! - [Chapter 3: Middleware][super::chapter_3] - Retry, auth, logging
