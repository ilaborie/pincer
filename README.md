# pincer

Declarative HTTP client for Rust.

[![CI](https://github.com/ilaborie/pincer/actions/workflows/ci.yml/badge.svg)](https://github.com/ilaborie/pincer/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE-MIT)

> **Proof of Concept**: This project is experimental and intended for learning and exploration. It is not recommended for production use.

> **AI-Assisted Development**: This project was built with assistance from Claude (Anthropic). Code, documentation, and tests were developed collaboratively between human and AI.

## Quick Start

```toml
[dependencies]
pincer = "0.1"
serde = { version = "1.0", features = ["derive"] }
tokio = { version = "1", features = ["full"] }
```

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

#[tokio::main]
async fn main() -> pincer::Result<()> {
    let client = UserApiClientBuilder::default().build()?;
    let user = client.get_user(42).await?;
    Ok(())
}
```

## Features

- **Declarative**: Define HTTP clients with proc-macro attributes
- **Type-safe**: Compile-time checked path, query, and body parameters
- **Async**: Built on tokio and hyper
- **Middleware**: Tower ecosystem integration (retry, auth, logging)
- **TLS**: Secure connections via rustls

## Learn More

- [Tutorial](https://docs.rs/pincer/latest/pincer/_tutorial/) - Step-by-step guide
- [API Docs](https://docs.rs/pincer) - Full API reference
- [Examples](./examples) - Working examples

## Inspiration

Inspired by [OpenFeign](https://github.com/OpenFeign/feign), the Java declarative HTTP client.

**Key differences from OpenFeign:**

- Rust's async/await instead of blocking calls
- Compile-time macro expansion instead of runtime proxies
- Tower middleware instead of interceptors
- Type-safe parameters enforced by the compiler

## Minimum Supported Rust Version

Rust 1.92 or later.

## License

MIT OR Apache-2.0
