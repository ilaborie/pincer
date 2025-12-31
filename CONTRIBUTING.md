# Contributing to pincer

Thank you for your interest in contributing to pincer! This document provides guidelines and instructions for contributing.

## Code of Conduct

Please be respectful and constructive in all interactions.

## Getting Started

1. Fork the repository
2. Clone your fork: `git clone https://github.com/YOUR_USERNAME/pincer.git`
3. Create a branch: `git checkout -b feature/your-feature-name`
4. Make your changes
5. Run the checks (see below)
6. Commit and push your changes
7. Open a Pull Request

## Development Requirements

- **Rust 1.92** or later (see `rust-version` in `Cargo.toml`)
- Run `rustup component add rustfmt clippy` for linting tools

## Running Checks

Before submitting a PR, ensure all checks pass:

```bash
# Format code
cargo fmt --all

# Run linter (warnings are errors)
cargo clippy --all-targets --all-features -- -D warnings

# Run tests
cargo test --all-features

# Check documentation builds without warnings
cargo doc --no-deps --all-features
```

## Code Style

### General Guidelines

- Follow Rust idioms and conventions
- Use `cargo fmt` for formatting
- Prefer `match` over `if let` when multiple patterns are involved
- Prefer `&str` over `String` when possible
- Avoid `unwrap()` in production code; use proper error handling
- In tests, use `expect("message")` instead of `unwrap()`

### Derive Order

When adding derives, use this order:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
```

For external derive macros, use the full qualified name:

```rust
#[derive(Debug, Clone, sqlx::FromRow, derive_more::Display)]
```

### Documentation

- Add doc comments to all public items
- Use `#[must_use]` for functions returning values that shouldn't be ignored
- Include examples in doc comments where helpful

## Project Structure

```
pincer/
├── lib/
│   ├── pincer-core/     # Core types: Request, Response, Error
│   ├── pincer-macro/    # Proc-macros: #[pincer], #[get], etc.
│   └── pincer/          # Main crate: HyperClient, middleware
└── examples/
    ├── github-api/         # GitHub API example
    └── wikipedia-api/      # Wikipedia API example
```

## Pull Request Guidelines

1. Keep PRs focused on a single change
2. Update documentation if needed
3. Add tests for new functionality
4. Ensure CI passes
5. Reference any related issues in the PR description

## Commit Messages

- Use conventional commit format when possible: `feat:`, `fix:`, `docs:`, `refactor:`, `test:`
- Keep the first line under 72 characters
- Provide context in the body if needed

## Reporting Issues

When reporting issues, please include:

- Rust version (`rustc --version`)
- Operating system
- Minimal reproduction code
- Expected vs actual behavior
- Full error message/stack trace if applicable

## Feature Requests

Feature requests are welcome! Please:

1. Check existing issues first
2. Describe the use case
3. Explain why existing features don't meet the need

## License

By contributing, you agree that your contributions will be licensed under the same terms as the project (MIT OR Apache-2.0).
