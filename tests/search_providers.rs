//! Integration tests for search providers.
//!
//! These tests run against live DuckDuckGo and Google servers. They are ignored by default
//! because they require network access and can be flaky (rate limits, CAPTCHAs).
//!
//! Run with: `cargo test --test search_providers -- --ignored`

use feroxyl::engine::{ddg, google, run_provider, SearchParams, SearchResult, TimeRange};

fn default_params(query: &str) -> SearchParams {
    SearchParams {
        query: query.to_string(),
        safesearch: feroxyl::engine::Safesearch::default(),
        time_range: TimeRange::default(),
        locale: "all".to_string(),
    }
}

fn assert_valid_results(results: &[SearchResult]) {
    assert!(!results.is_empty(), "expected at least one result");
    for r in results {
        assert!(!r.title.is_empty(), "result title should not be empty");
        assert!(!r.url.is_empty(), "result url should not be empty");
        assert!(r.url.starts_with("http"), "result url should be absolute");
    }
}

#[tokio::test]
#[ignore = "requires network access; run with: cargo test --test search_providers -- --ignored"]
async fn duckduckgo_search_returns_results() {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("reqwest client");

    let mut provider = ddg::DuckDuckGo::new();
    let params = default_params("rust programming");

    let results = run_provider(&mut provider, &client, params)
        .await
        .expect("DuckDuckGo search should succeed");

    assert_valid_results(&results);
}

#[tokio::test]
#[ignore = "requires network access; run with: cargo test --test search_providers -- --ignored"]
async fn duckduckgo_search_with_time_range() {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("reqwest client");

    let mut provider = ddg::DuckDuckGo::new();
    let mut params = default_params("searxng");
    params.time_range = TimeRange::Week;

    let results = run_provider(&mut provider, &client, params)
        .await
        .expect("DuckDuckGo search with time range should succeed");

    assert_valid_results(&results);
}

#[tokio::test]
#[ignore = "requires network access; run with: cargo test --test search_providers -- --ignored"]
async fn google_search_returns_results() {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("reqwest client");

    let mut provider = google::Google::new();
    let params = default_params("rust programming");

    let results = run_provider(&mut provider, &client, params)
        .await
        .expect("Google search should succeed");

    assert_valid_results(&results);
}

#[tokio::test]
#[ignore = "requires network access; run with: cargo test --test search_providers -- --ignored"]
async fn google_search_with_time_range_and_safesearch() {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("reqwest client");

    let mut provider = google::Google::new();
    let mut params = default_params("open source search");
    params.time_range = TimeRange::Month;
    params.safesearch = feroxyl::engine::Safesearch::Moderate;

    let results = run_provider(&mut provider, &client, params)
        .await
        .expect("Google search with filters should succeed");

    assert_valid_results(&results);
}
