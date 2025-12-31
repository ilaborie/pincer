# pincer

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
