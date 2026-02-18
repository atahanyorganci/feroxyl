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

/// Trait for search providers. HTTP execution is handled externally.
pub trait SearchProvider {
    type Params;

    fn build_request(
        &self,
        params: Self::Params,
    ) -> Result<reqwest::Request, Box<dyn Error + Send + Sync>>;

    fn parse_response(&self, body: &str)
        -> Result<Vec<SearchResult>, Box<dyn Error + Send + Sync>>;
}
