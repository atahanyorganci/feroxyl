//! Search engine implementations
//!
//! Common parameters follow SearXNG's RequestParams / SearchQuery model.

use std::error::Error;

pub mod ddg;
pub mod google;

/// Unified search result type for all providers
#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub content: Option<String>,
}

/// Time range filter for search results (SearXNG: time_range).
/// Maps to engine-specific codes (e.g. DDG: d/w/m/y, Google: qdr:d/w/m/y).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TimeRange {
    #[default]
    Any,
    Day,
    Week,
    Month,
    Year,
}

/// Safe search filter level (SearXNG: safesearch 0/1/2).
/// 0: off, 1: moderate, 2: strict.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Safesearch {
    #[default]
    Off,
    Moderate,
    Strict,
}

impl Safesearch {
    /// From numeric level (0, 1, 2)
    pub fn from_level(level: u8) -> Self {
        match level {
            1 => Safesearch::Moderate,
            2 => Safesearch::Strict,
            _ => Safesearch::Off,
        }
    }
}

/// Common search parameters shared by all providers.
/// Mirrors SearXNG's RequestParams: query, safesearch, time_range, searxng_locale.
#[derive(Debug, Clone)]
pub struct SearchParams {
    /// Search query string
    pub query: String,
    /// Safe search filter
    pub safesearch: Safesearch,
    /// Optional time range filter
    pub time_range: TimeRange,
    /// Locale/language (e.g. "all", "en", "en-US"). "all" = no language/region filter.
    pub locale: String,
}

impl Default for SearchParams {
    fn default() -> Self {
        Self {
            query: String::new(),
            safesearch: Safesearch::default(),
            time_range: TimeRange::default(),
            locale: "all".to_string(),
        }
    }
}

/// Runs a search provider until completion, executing HTTP requests with the given client.
pub async fn run_provider<P: SearchProvider>(
    provider: &mut P,
    client: &reqwest::Client,
    params: SearchParams,
) -> Result<Vec<SearchResult>, Box<dyn Error + Send + Sync>> {
    let mut all = Vec::new();
    let mut params = Some(params);

    loop {
        let req = match provider.build_request(params.take()) {
            Ok(Some(r)) => r,
            Ok(None) => break,
            Err(e) => return Err(e),
        };
        let response = client.execute(req).await?;
        let body = response.text().await?;
        provider.parse_response(&body)?;

        while let Some(r) = provider.results() {
            match r {
                Ok(sr) => all.extend(sr),
                Err(e) => return Err(e),
            }
        }
    }
    Ok(all)
}

/// Trait for search providers as a state machine. HTTP execution is handled externally.
pub trait SearchProvider {
    /// Build the next request to send. Returns None when no more requests.
    /// params: Some on first call, None on continuation (provider uses stored state).
    fn build_request(
        &mut self,
        params: Option<SearchParams>,
    ) -> Result<Option<reqwest::Request>, Box<dyn Error + Send + Sync>>;

    /// Parse an HTTP response body. Updates internal state (e.g. result queue).
    fn parse_response(&mut self, body: &str) -> Result<(), Box<dyn Error + Send + Sync>>;

    /// Yield the next result. None when no more results; caller loops back to build_request.
    fn results(&mut self) -> Option<Result<Vec<SearchResult>, Box<dyn Error + Send + Sync>>>;
}
