# pincer-macro

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
