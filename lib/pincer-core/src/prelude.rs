//! Prelude module for convenient imports.
//!
//! This module re-exports the most commonly used types and functions
//! for easy glob importing:
//!
//! ```ignore
//! use pincer_core::prelude::*;
//! ```

pub use crate::{
    ContentType, DefaultErrorDecoder, Error, ErrorDecoder, Form, HttpClient, HttpClientExt, Method,
    Part, Request, RequestBuilder, Response, Result, from_json, to_form, to_json,
};
