# pincer

[![Crates.io](https://img.shields.io/crates/v/pincer.svg)](https://crates.io/crates/pincer)
[![docs.rs](https://img.shields.io/docsrs/pincer)](https://docs.rs/pincer)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](https://github.com/ilaborie/pincer/blob/main/LICENSE-MIT)

Declarative HTTP client for Rust.

## Quick Start

```rust
use pincer::prelude::*;

#[derive(Debug, Deserialize)]
pub struct User {
    pub id: u64,
    pub name: String,
}

#[pincer(url = "https://api.example.com")]
pub trait UserApi {
    #[get("/users/{id}")]
    async fn get_user(&self, #[path] id: u64) -> pincer::Result<User>;
}

let client = UserApiClientBuilder::default().build()?;
let user = client.get_user(42).await?;
```

See the [tutorial](https://docs.rs/pincer/latest/pincer/_tutorial/) for more.

## License

MIT OR Apache-2.0
