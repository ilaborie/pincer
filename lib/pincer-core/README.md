# pincer-core

[![Crates.io](https://img.shields.io/crates/v/pincer-core.svg)](https://crates.io/crates/pincer-core)
[![docs.rs](https://img.shields.io/docsrs/pincer-core)](https://docs.rs/pincer-core)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](https://github.com/ilaborie/pincer/blob/main/LICENSE-MIT)

Core types and traits for the [pincer](https://crates.io/crates/pincer) HTTP client.

## Overview

This crate provides the foundational types used by pincer:

- [`Request`](https://docs.rs/pincer-core/latest/pincer_core/struct.Request.html) / [`Response`](https://docs.rs/pincer-core/latest/pincer_core/struct.Response.html) - HTTP request/response types
- [`HttpClient`](https://docs.rs/pincer-core/latest/pincer_core/trait.HttpClient.html) / [`PincerClient`](https://docs.rs/pincer-core/latest/pincer_core/trait.PincerClient.html) - Client traits
- [`Error`](https://docs.rs/pincer-core/latest/pincer_core/enum.Error.html) / [`Result`](https://docs.rs/pincer-core/latest/pincer_core/type.Result.html) - Error handling
- [`ToQueryPairs`](https://docs.rs/pincer-core/latest/pincer_core/trait.ToQueryPairs.html) - Query parameter serialization

## Usage

Most users should use the main [`pincer`](https://crates.io/crates/pincer) crate, which re-exports these types.

Use `pincer-core` directly if you need:
- Custom `PincerClient` implementations
- Core types without the HTTP client

## License

MIT OR Apache-2.0
