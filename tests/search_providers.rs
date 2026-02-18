//! Integration tests for search providers.
//!
//! These tests run against live DuckDuckGo and Google servers. They are ignored by default
//! because they require network access and can be flaky (rate limits, CAPTCHAs).
//!
//! Run with: `cargo test --test search_providers -- --ignored`

use feroxyl::engine::{
    brave, ddg, google, run_meta_search, run_provider, RankedSearchResult, SearchParams,
    SearchResult, TimeRange,
};

fn default_params(query: &str) -> SearchParams {
    SearchParams {
        query: query.to_string(),
        safesearch: feroxyl::engine::Safesearch::default(),
        time_range: TimeRange::default(),
        locale: feroxyl::engine::Locale::default(),
    }
}

fn assert_valid_ranked_results(results: &[RankedSearchResult]) {
    assert!(!results.is_empty(), "expected at least one result");
    for r in results {
        assert!(!r.title.is_empty(), "result title should not be empty");
        assert!(!r.url.is_empty(), "result url should not be empty");
        assert!(r.url.starts_with("http"), "result url should be absolute");
        assert!(r.score > 0.0, "result score should be positive");
        assert!(
            !r.position.is_empty(),
            "result should have at least one engine position"
        );
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
    let params = default_params("rust programming");
    let results = run_provider::<ddg::DuckDuckGo>(&params)
        .await
        .expect("DuckDuckGo search should succeed");

    assert_valid_results(&results);
}

#[tokio::test]
#[ignore = "requires network access; run with: cargo test --test search_providers -- --ignored"]
async fn meta_search_returns_merged_results() {
    let params = default_params("rust programming");

    let results = run_meta_search(&params)
        .await
        .expect("meta search should succeed");

    assert_valid_ranked_results(&results);
    // Results should be sorted by score descending
    for w in results.windows(2) {
        assert!(
            w[0].score >= w[1].score,
            "results should be sorted by score"
        );
    }
}

#[tokio::test]
#[ignore = "requires network access; run with: cargo test --test search_providers -- --ignored"]
async fn duckduckgo_search_with_time_range() {
    let mut params = default_params("searxng");
    params.time_range = TimeRange::Week;

    let results = run_provider::<ddg::DuckDuckGo>(&params)
        .await
        .expect("DuckDuckGo search with time range should succeed");

    assert_valid_results(&results);
}

#[tokio::test]
#[ignore = "requires network access; run with: cargo test --test search_providers -- --ignored"]
async fn google_search_returns_results() {
    let params = default_params("rust programming");

    let results = run_provider::<google::Google>(&params)
        .await
        .expect("Google search should succeed");

    assert_valid_results(&results);
}

#[tokio::test]
#[ignore = "requires network access; run with: cargo test --test search_providers -- --ignored"]
async fn brave_search_returns_results() {
    let params = default_params("rust programming");

    let results = run_provider::<brave::Brave>(&params)
        .await
        .expect("Brave search should succeed");

    assert_valid_results(&results);
}

#[tokio::test]
#[ignore = "requires network access; run with: cargo test --test search_providers -- --ignored"]
async fn google_search_with_time_range_and_safesearch() {
    let mut params = default_params("open source search");
    params.time_range = TimeRange::Month;
    params.safesearch = feroxyl::engine::Safesearch::Moderate;

    let results = run_provider::<google::Google>(&params)
        .await
        .expect("Google search with filters should succeed");

    assert_valid_results(&results);
}
