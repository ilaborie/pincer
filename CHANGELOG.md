# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2025-01-01

### Added

- Initial release of pincer
- **Declarative HTTP client** using proc-macros
  - `#[pincer]` attribute for defining API clients
  - HTTP method macros: `#[get]`, `#[post]`, `#[put]`, `#[delete]`, `#[patch]`, `#[head]`, `#[options]`
  - Custom HTTP method support via `#[http("METHOD /path")]`
- **Parameter attributes**
  - `#[path]` - Path parameters with URL encoding
  - `#[query]` - Query string parameters (supports `Option<T>`, `Vec<T>`, and structs)
  - `#[body]` - JSON request body serialization
  - `#[form]` - Form URL-encoded body serialization
  - `#[header("Name")]` - Single header injection
  - `#[headers]` - Multiple headers via `HashMap`
- **Error handling**
  - Comprehensive error types with response body access
  - Contextual JSON deserialization errors with path information
- **Tower middleware integration**
  - Retry middleware (`middleware-retry` feature)
  - Logging middleware (`middleware-logging` feature)
  - Bearer authentication (`middleware-bearer-auth` feature)
  - Concurrency limiting (`middleware-concurrency` feature)
  - Generic `.layer()` API for custom Tower layers
- **Optional tower-http integration**
  - Trace layer (`tower-http-trace` feature)
  - Follow redirect layer (`tower-http-follow-redirect` feature)
  - Compression layer (`tower-http-compression` feature)
- **Networking**
  - TLS support via rustls
  - Connection pooling via hyper-util
  - Configurable timeouts
- **Serialization**
  - JSON via serde_json
  - Form URL-encoded via serde_urlencoded
  - Query string via serde_html_form

### Documentation

- GitHub API example demonstrating declarative clients
- Wikipedia API example with lazy pagination

[Unreleased]: https://github.com/ilaborie/pincer/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/ilaborie/pincer/releases/tag/v0.1.0
