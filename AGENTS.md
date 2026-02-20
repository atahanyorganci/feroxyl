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
│   ├── lib.rs              # Library root; re-exports engine, scrape, server
│   ├── scrape.rs           # HTML to Markdown conversion
│   ├── server/
│   │   ├── mod.rs          # App factory, shared query params (SearchQuery, etc.)
│   │   ├── api.rs          # JSON API routes (/api/search, /api/search/image, /api/scrape)
│   │   └── view.rs         # HTML views (/, /search)
│   └── engine/
│       ├── mod.rs          # SearchProvider, ImageSearchProvider, SearchResult, Provider enum
│       ├── ddg/
│       │   ├── mod.rs      # Shared DDG utilities (extr, locale_to_ddg_region, build_vqd_request)
│       │   ├── web.rs      # DuckDuckGo HTML search
│       │   └── news.rs    # DuckDuckGo news search
│       ├── google.rs       # Google search (HTML scraping)
│       ├── brave.rs        # Brave search
│       ├── startpage.rs    # Startpage search
│       ├── bing.rs         # Bing search
│       └── bing_images.rs  # Bing image search (ImageSearchProvider)
├── tests/
│   ├── search_providers.rs # Provider integration tests
│   └── api.rs              # API integration tests
└── vendor/
    ├── frontend/           # React/TSX UI; design system in src/index.css
    └── searxng/            # Git submodule; reference for porting engines
```

### Key Files

| File                        | Purpose                                                                           |
| --------------------------- | --------------------------------------------------------------------------------- |
| `src/engine/mod.rs`         | Defines `SearchProvider`, `ImageSearchProvider`, `SearchResult`, `Provider` enum. |
| `src/engine/ddg/mod.rs`     | Shared DDG utilities: `extr`, `locale_to_ddg_region`, `build_vqd_request`.        |
| `src/engine/ddg/web.rs`     | DuckDuckGo web search; VQD token flow, pagination, time range filters.            |
| `src/engine/ddg/news.rs`    | DuckDuckGo news search; JSON API at news.js.                                      |
| `src/engine/google.rs`      | Google implementation; parses async/arc HTML format.                              |
| `src/engine/brave.rs`       | Brave search implementation.                                                      |
| `src/engine/startpage.rs`   | Startpage search implementation.                                                  |
| `src/engine/bing.rs`        | Bing search implementation.                                                       |
| `src/engine/bing_images.rs` | Bing image search; implements `ImageSearchProvider`.                              |
| `src/server/mod.rs`         | App factory `create_app`; shared `SearchQuery`, `ImageSearchQuery`.               |
| `src/server/api.rs`         | JSON API: `/api/search`, `/api/search/image`, `/api/scrape/*path`.                |
| `src/server/view.rs`        | HTML views: `/` (index), `/search` (streaming results).                           |
| `src/main.rs`               | CLI; demonstrates provider usage.                                                 |

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

### Import/Export

- No aliases. Never `use api::routes as api_routes;` or similar aliases. Always use the full path to the module, or import the elements directly.
- Keep public exports to a minimum. Only export the types and functions that are needed by the public API.
- Prefer importing instead of writing `create::engine::TimeRange` instead use `use create::engine::TimeRange;`.

### External IO

Code should be structured so that IO operations are handled externally to how the business logic, search providers, are implemented. This is to allow for the business logic to be tested in isolation, and to allow for the IO operations to be mocked for testing.

### Formatting and Linting

Always run formatters and linters to ensure code is formatted and linted correctly.

- Formatter `nix fmt`, this command formats all sources in the repository.
- Rust linter: `nix develop --command cargo clippy`

### Errors

Using `thiserror` create custom descriptive error types that are used throughout the codebase.

## Color Scheme and Design System

**Design philosophy:** Ink on paper. Authoritative. Calm. Editorial.

### Brand Palette

| Token       | Light                     | Dark          | Usage                                |
| ----------- | ------------------------- | ------------- | ------------------------------------ |
| **Oxblood** | `#7F1D1D` / `0 63% 31%`   | `0 55% 45%`   | Primary actions, links, focus rings  |
| **Gold**    | `#D4A373` / `30 53% 64%`  | `30 48% 55%`  | Accents, decorative lines, selection |
| **Paper**   | `#F5F1E8` / `43 47% 94%`  | `222 25% 12%` | Cards, secondary surfaces            |
| **Ink**     | `#111827` / `221 39% 11%` | `40 30% 92%`  | Primary text                         |

### Surfaces and Text

- **Background:** Warm off-white (`40 60% 98%`) in light; deep ink (`222 30% 8%`) in dark.
- **Muted text:** `220 9% 46%` (light) / `40 15% 60%` (dark).
- **Borders:** `42 20% 87%` (light) / `222 18% 22%` (dark).

### Typography

- **Serif:** Literata (headings, editorial emphasis).
- **Sans:** Source Sans 3 (body, UI).
- **Mono:** JetBrains Mono (URLs, keys, code).

### Decorative Elements

- **Top accent rule:** `linear-gradient(90deg, hsl(var(--oxblood)), hsl(var(--gold)) 60%, transparent)` or `rgb(127 29 29)` → `rgb(217 119 6)`.
- **Ruled line:** Same gradient for section dividers.

### Tailwind Mapping (Server-Side)

When using Tailwind via CDN in `src/server/view.rs` (no CSS variables), use these equivalents:

- Oxblood: `red-900`, `red-800` (hover).
- Gold: `amber-500`.
- Paper/surface: `stone-50`, `stone-100`, `stone-200`.
- Ink: `slate-900`, `slate-500` (muted).

## Markup Guidelines

### Server-Side HTML (`src/server/view.rs`)

- Use the **`markup`** crate for type-safe HTML generation.
- Define reusable components with `markup::define! { ComponentName { ... } }`.
- Build pages with `markup::new! { ... }` and always start with `@markup::doctype()`.
- Use `markup::Render` and `.render(&mut buf)` or `.to_string()` for output.
- Include Tailwind via CDN: `script[src="https://cdn.jsdelivr.net/npm/@tailwindcss/browser@4"]`.
- Escape backticks when injecting HTML into `<script>` templates: `html.replace('`', r"\`")`.
- Use Unicode entities for typography: `\u{2026}` (…), `\u{00a0}` (non-breaking space).

### Frontend (React/TSX in `vendor/frontend/`)

- Use Tailwind utility classes with CSS variables: `bg-background`, `text-foreground`, `text-oxblood`, `bg-paper`, `border-border`.
- Use semantic HTML: `main`, `header`, `footer`, `kbd` for keyboard hints.
- Apply design tokens: `font-serif`, `font-sans-ui`, `font-mono-url` for typography.
- Use `ruled-line` for section dividers; `animate-fade-in`, `animate-slide-up` for entry animations.
