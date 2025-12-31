# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Pincer is a declarative HTTP client for Rust, inspired by OpenFeign. It uses proc-macros to generate HTTP clients from trait definitions with async/await and Tower middleware support.

## Development Commands (mise tasks)

This project uses [mise](https://mise.jdx.dev/) for task management.

```bash
mise check    # REQUIRED before commit/push: format check, lint, all tests
mise test     # Run unit tests (nextest) and doctests
mise lint     # Run clippy with warnings as errors
mise format   # Auto-format code
mise fix      # Auto-format + apply clippy fixes
```

**Pre-commit requirement:** Always run `mise check` before committing or pushing. This runs format checking, linting, and all tests (including ignored doctests).

For running a single test:
```bash
cargo nextest run --all-features test_name
```

## Architecture

### Crate Structure

- **pincer-core** (`lib/pincer-core/`) - Core types: `Request`, `Response`, `Error`, `HttpClient` trait, `ToQueryPairs` trait
- **pincer-macro** (`lib/pincer-macro/`) - Proc-macros: `#[pincer]`, `#[get]`, `#[post]`, `#[derive(Query)]`, etc.
- **pincer** (`lib/pincer/`) - Main crate: `HyperClient`, middleware layers, and re-exports

### How the Macro System Works

1. `#[pincer(url = "...")]` on a trait generates:
   - A clean trait without pincer attributes
   - A client struct (e.g., `UserApiClient`)
   - A builder struct (e.g., `UserApiClientBuilder`)

2. Method attributes (`#[get]`, `#[post]`, etc.) define HTTP operations
3. Parameter attributes (`#[path]`, `#[query]`, `#[body]`, `#[header]`, `#[form]`) control parameter handling

### Middleware System

Middleware uses Tower's `Layer` trait. Feature-gated helpers available:
- `middleware-core`: timeout, retry, bearer/basic auth, concurrency
- `middleware-resilience`: rate limiting, circuit breaker
- `middleware-full`: all middleware including metrics

## Design Principles

Follow these principles in order of priority:

- **KISS** (Keep It Simple, Stupid) - Choose the simplest solution that works. Complexity is the enemy of maintainability.
- **YAGNI** (You Aren't Gonna Need It) - Don't add functionality until it's actually needed. Avoid speculative generalization.
- **DRY** (Don't Repeat Yourself) - Extract common logic only when duplication is proven problematic. Three similar lines are better than a premature abstraction.

Apply SOLID principles pragmatically:
- **Single Responsibility** - Each module/struct should have one reason to change
- **Open/Closed** - Prefer extension (traits, generics) over modification
- **Liskov Substitution** - Subtypes must be substitutable for their base types
- **Interface Segregation** - Keep traits focused and minimal
- **Dependency Inversion** - Depend on abstractions (traits), not concretions

Clean Code & Architecture guidelines:
- Meaningful names that reveal intent
- Small, focused functions that do one thing well
- Separate business logic from infrastructure concerns
- Use domain-driven design: model the problem domain with clear bounded contexts

## Code Style

- Derive order: `Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize`
- External derives use full path: `sqlx::FromRow`, `derive_more::Debug`
- Prefer turbofish (`::<T>()`) over type annotations
- Use `Result` and `Option` appropriately; avoid `unwrap()` in production code
- In tests, use `expect("message")` instead of `unwrap()`
- Prefer `&str` over `String`, `as_ref()` over `clone()` when possible

## Important: No AI Attribution in Code

Never include mentions of "Claude", "Anthropic", or any AI assistant in:
- Source code or comments
- Commit messages
- Pull request descriptions
- Documentation files

The codebase should appear as standard human-authored code. This maintains professionalism and avoids unnecessary attribution noise.

## Minimum Supported Rust Version

Rust 1.92 (edition 2024)
