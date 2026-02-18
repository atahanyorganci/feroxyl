//! Search engine implementations

use std::error::Error;

pub mod ddg;
pub mod google;

/// Unified search result type for all providers
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub content: Option<String>,
}

/// Trait for search providers as a state machine. HTTP execution is handled externally.
pub trait SearchProvider {
    type Params;

    /// Build the next request to send. Returns None when no more requests.
    /// params: Some on first call, None on continuation (provider uses stored state).
    fn build_request(
        &mut self,
        params: Option<Self::Params>,
    ) -> Result<Option<reqwest::Request>, Box<dyn Error + Send + Sync>>;

    /// Parse an HTTP response body. Updates internal state (e.g. result queue).
    fn parse_response(&mut self, body: &str) -> Result<(), Box<dyn Error + Send + Sync>>;

    /// Yield the next result. None = provider needs another HTTP request (call build_request again).
    fn results(&mut self) -> Option<Result<SearchResult, Box<dyn Error + Send + Sync>>>;
}
