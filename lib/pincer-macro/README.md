# pincer-macro

[![Crates.io](https://img.shields.io/crates/v/pincer-macro.svg)](https://crates.io/crates/pincer-macro)
[![docs.rs](https://img.shields.io/docsrs/pincer-macro)](https://docs.rs/pincer-macro)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](https://github.com/ilaborie/pincer/blob/main/LICENSE-MIT)

Proc-macros for the [pincer](https://crates.io/crates/pincer) HTTP client.

## Overview

This crate provides the proc-macro attributes:

- `#[pincer]` - Define an API client trait
- `#[get]`, `#[post]`, `#[put]`, `#[delete]`, `#[patch]`, `#[head]`, `#[options]` - HTTP methods
- `#[http]` - Custom HTTP methods
- `#[derive(Query)]` - Query parameter serialization

## Usage

Most users should use the main [`pincer`](https://crates.io/crates/pincer) crate, which re-exports these macros.

```rust
use pincer::prelude::*;

#[pincer(url = "https://api.example.com")]
pub trait MyApi {
    #[get("/users/{id}")]
    async fn get_user(&self, #[path] id: u64) -> pincer::Result<User>;
}
```

## License

MIT OR Apache-2.0
