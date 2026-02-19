# AGENTS.md

Guidance for AI agents and developers working on this codebase. Use this file to craft better prompts and understand project conventions.

## Project Overview

**Meta Search Engine** is a Rust library and server that aggregates search results from multiple providers (DuckDuckGo, Google, Brave, Startpage, Bing, etc.). It uses HTML scraping rather than official APIs, with engine implementations ported from [SearXNG](vendor/searxng/) as reference.

- **Package name:** `feroxyl` (in Cargo.toml)
- **Edition:** Rust 2021
- **Runtime:** Async with Tokio

## Repository Structure

```
feroxyl/
├── Cargo.toml
├── Cargo.lock
├── flake.nix
├── crane.nix
├── src/
│   ├── main.rs             # CLI entry point; runs providers and prints results
│   ├── lib.rs              # Library root; re-exports api, engine, scrape
│   ├── api.rs              # HTTP API routes and app factory (Axum)
│   ├── scrape.rs           # HTML to Markdown conversion
│   └── engine/
│       ├── mod.rs          # SearchProvider, ImageSearchProvider, SearchResult, Provider enum
│       ├── ddg.rs          # DuckDuckGo HTML search
│       ├── google.rs       # Google search (HTML scraping)
│       ├── brave.rs        # Brave search
│       ├── startpage.rs    # Startpage search
│       ├── bing.rs         # Bing search
│       └── bing_images.rs  # Bing image search (ImageSearchProvider)
├── tests/
│   ├── search_providers.rs # Provider integration tests
│   └── api.rs              # API integration tests
└── vendor/
    └── searxng/            # Git submodule; reference for porting engines
```

### Key Files

| File                        | Purpose                                                                               |
| --------------------------- | ------------------------------------------------------------------------------------- |
| `src/engine/mod.rs`         | Defines `SearchProvider`, `ImageSearchProvider`, `SearchResult`, `Provider` enum.     |
| `src/engine/ddg.rs`         | DuckDuckGo implementation; VQD token flow, pagination, time range filters.            |
| `src/engine/google.rs`      | Google implementation; parses async/arc HTML format.                                  |
| `src/engine/brave.rs`       | Brave search implementation.                                                          |
| `src/engine/startpage.rs`   | Startpage search implementation.                                                      |
| `src/engine/bing.rs`        | Bing search implementation.                                                           |
| `src/engine/bing_images.rs` | Bing image search; implements `ImageSearchProvider`.                                  |
| `src/api.rs`                | Axum routes: `/search`, `/image`, `/health`; `run_meta_search`, `run_image_provider`. |
| `src/main.rs`               | CLI; demonstrates provider usage.                                                     |

## Architecture

### SearchProvider Trait

Providers are **state machines**. HTTP is executed externally; providers only build requests and parse responses.

```rust
pub trait SearchProvider
where
    Self: Default,
{
    fn name() -> &'static str;

    fn build_request(&mut self, params: &SearchParams) -> Result<reqwest::Request, ...>;
    fn parse_response(&mut self, body: &str) -> Result<(), ...>;
    fn results(&mut self) -> Option<Result<Vec<SearchResult>, ...>>;
}
```

**Flow:** `build_request(params)` → execute HTTP → `parse_response(body)` → `results()` until `Some(...)` or done; if `results()` returns `None`, loop back to `build_request(params)` for next page.

### ImageSearchProvider Trait

Same state-machine flow as `SearchProvider` but yields `ImageResult` (url, img_src, thumbnail_src, title, etc.). Use `run_image_provider` for execution.

### Provider Enum and Meta Search

The `Provider` enum (`DuckDuckGo`, `Google`, `Brave`, `Startpage`, `Bing`) dispatches to each engine. `run_meta_search` runs multiple providers in parallel and merges results by URL with ranking (`RankedSearchResult`).

### SearchParams

Common parameters (mirrors SearXNG): `query`, `safesearch`, `time_range`, `locale`. Types: `TimeRange` (Any/Day/Week/Month/Year), `Safesearch` (Off/Moderate/Strict), `Locale` (All, EnUS, EnGB, TrTR, Other).

### Adding a New Engine

1. Create `src/engine/<name>.rs` implementing `SearchProvider` (or `ImageSearchProvider` for image engines).
2. Add `mod <name>;` and `pub use <name>::<Type>;` in `src/engine/mod.rs`.
3. Add the provider to the `Provider` enum and its `run`/`name` match arms.
4. Use `vendor/searxng/searx/engines/*.py` as reference for URL construction and HTML parsing.

## Coding Style

### External IO

Code should be structured so that IO operations are handled externally to how the business logic, search providers, are implemented. This is to allow for the business logic to be tested in isolation, and to allow for the IO operations to be mocked for testing.

### Formatting and Linting

Always run formatters and linters to ensure code is formatted and linted correctly.

- Rust formatter: `nix develop --command "cargo fmt"`
- Rust linter: `nix develop --command "cargo clippy"`
- Nix formatter: `nix fmt`

### Errors

Using `thiserror` create custom descriptive error types that are used throughout the codebase.
