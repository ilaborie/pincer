//! Prelude module for convenient imports.
//!
//! This module re-exports the most commonly used types, functions, and macros
//! for easy glob importing:
//!
//! ```ignore
//! use pincer::prelude::*;
//! ```

pub use crate::{
    ApiClient, ClientConfig, ContentType, Error, Form, HttpClient, HttpClientExt, HyperClient,
    Method, Part, PincerClient, Query, Request, RequestBuilder, Response, Result, StatusCode,
    ToQueryPairs, delete, from_json, get, head, header, http, options, patch, pincer, post, put,
    to_form, to_json,
};
pub use serde::{Deserialize, Serialize};
