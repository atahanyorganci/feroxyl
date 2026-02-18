# AGENTS.md

Guidance for AI agents and developers working on this codebase. Use this file to craft better prompts and understand project conventions.

## Project Overview

**Meta Search Engine** is a Rust library and server aggregates search results from multiple providers (DuckDuckGo, Google, etc.). It uses HTML scraping rather than official APIs, with engine implementations ported from [SearXNG](vendor/searxng/) as reference.

- **Package name:** `quick-start` (in Cargo.toml)
- **Edition:** Rust 2021
- **Runtime:** Async with Tokio

## Repository Structure

```
feroxyl/
├── Cargo.toml
├── Cargo.lock
├── src/
│   ├── main.rs             # CLI entry point; runs providers and prints results
│   ├── lib.rs              # Library root; re-exports engine module
│   └── engine/
│       ├── mod.rs          # SearchProvider trait, SearchResult type, engine submodules
│       ├── ddg.rs          # DuckDuckGo HTML search (port of SearXNG duckduckgo.py)
│       └── google.rs       # Google search (HTML scraping)
└── vendor/
    └── searxng/            # Git submodule; reference for porting engines
```

### Key Files

| File                   | Purpose                                                                         |
| ---------------------- | ------------------------------------------------------------------------------- |
| `src/engine/mod.rs`    | Defines `SearchProvider` trait and `SearchResult` struct. Add new engines here. |
| `src/engine/ddg.rs`    | DuckDuckGo implementation; uses VQD token flow, pagination, time range filters. |
| `src/engine/google.rs` | Google implementation; parses async/arc HTML format.                            |
| `src/main.rs`          | Demonstrates `run_provider` loop; HTTP execution is external to providers.      |

## Architecture

### SearchProvider Trait

Providers are **state machines**. HTTP is executed externally; providers only build requests and parse responses.

```rust
pub trait SearchProvider {
 type Params;

 fn build_request(&mut self, params: Option<Self::Params>) -> Result<Option<Request>, ...>;
 fn parse_response(&mut self, body: &str) -> Result<(), ...>;
 fn results(&mut self) -> Option<Result<Vec<SearchResult>, ...>>;
}
```

**Flow:** `build_request(params)` → execute HTTP → `parse_response(body)` → `results()` until `None` → loop back to `build_request(None)` for next page; `build_request` returns `None` when done.

### Adding a New Engine

1. Create `src/engine/<name>.rs` implementing `SearchProvider`.
2. Add `pub mod <name>;` in `src/engine/mod.rs`.
3. Use `vendor/searxng/searx/engines/*.py` as reference for URL construction and HTML parsing.

## Coding Style

### External IO

Code should be structured so that IO operations are handled externally to how the business logic, search providers, are implemented. This is to allow for the business logic to be tested in isolation, and to allow for the IO operations to be mocked for testing.

### Formatting and Linting

Always run `cargo fmt` and `cargo clippy` to ensure code is formatted and linted correctly.

### Errors

Using `thiserror` create custom descriptive error types that are used throughout the codebase.
