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
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
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
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Safesearch {
    #[default]
    Off,
    Moderate,
    Strict,
}

/// Locale/language for search results (BCP 47 style).
/// Mirrors SearXNG's searxng_locale.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum Locale {
    /// No language/region filter (all locales).
    #[default]
    All,
    /// English (United States)
    EnUS,
    /// English (United Kingdom)
    EnGB,
    /// Turkish (Turkey)
    TrTR,
    /// Custom locale tag (e.g. "de-DE", "fr-FR").
    Other(String),
}

impl Locale {
    /// BCP 47 tag for this locale (e.g. "en-US", "tr-TR").
    /// Returns "all" for `All`.
    pub fn as_str(&self) -> &str {
        match self {
            Locale::All => "all",
            Locale::EnUS => "en-US",
            Locale::EnGB => "en-GB",
            Locale::TrTR => "tr-TR",
            Locale::Other(s) => s.as_str(),
        }
    }
}

impl std::fmt::Display for Locale {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for Locale {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "" | "all" => Locale::All,
            "en-US" | "en_us" => Locale::EnUS,
            "en-GB" | "en_GB" | "en-UK" | "en_UK" => Locale::EnGB,
            "tr-TR" | "tr_TR" => Locale::TrTR,
            other => Locale::Other(other.to_string()),
        })
    }
}

/// Error when parsing an invalid locale string.
#[derive(Debug, thiserror::Error)]
#[error("invalid locale: {0}")]
pub struct InvalidLocale(pub String);

impl TryFrom<String> for Locale {
    type Error = InvalidLocale;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        match s.as_str() {
            "all" => Ok(Locale::All),
            "en-US" | "en_us" => Ok(Locale::EnUS),
            "en-GB" | "en_GB" | "en-UK" | "en_UK" => Ok(Locale::EnGB),
            "tr-TR" | "tr_TR" => Ok(Locale::TrTR),
            other => Err(InvalidLocale(other.to_string())),
        }
    }
}

impl From<Locale> for String {
    fn from(locale: Locale) -> Self {
        locale.as_str().to_string()
    }
}

/// Common search parameters shared by all providers.
/// Mirrors SearXNG's RequestParams: query, safesearch, time_range, searxng_locale.
#[derive(Debug, Clone, Default)]
pub struct SearchParams {
    /// Search query string
    pub query: String,
    /// Safe search filter
    pub safesearch: Safesearch,
    /// Optional time range filter
    pub time_range: TimeRange,
    /// Locale/language for search results.
    pub locale: Locale,
}

/// Runs a search provider until completion, executing HTTP requests with the given client.
pub async fn run_provider<P: SearchProvider>(
    params: &SearchParams,
) -> Result<Vec<SearchResult>, Box<dyn Error + Send + Sync>> {
    let mut provider = P::default();
    let client = reqwest::Client::new();
    loop {
        let request = provider.build_request(&params)?;
        let response = client.execute(request).await?;
        let body = response.text().await?;
        provider.parse_response(&body)?;

        if let Some(r) = provider.results() {
            break r;
        }
    }
}

/// Trait for search providers as a state machine. HTTP execution is handled externally.
pub trait SearchProvider
where
    Self: Default,
{
    /// Build the next request to send. Returns None when no more requests.
    /// params: Some on first call, None on continuation (provider uses stored state).
    fn build_request(
        &mut self,
        params: &SearchParams,
    ) -> Result<reqwest::Request, Box<dyn Error + Send + Sync>>;

    /// Parse an HTTP response body. Updates internal state (e.g. result queue).
    fn parse_response(&mut self, body: &str) -> Result<(), Box<dyn Error + Send + Sync>>;

    /// Yield the next result. None when no more results; caller loops back to build_request.
    fn results(&mut self) -> Option<Result<Vec<SearchResult>, Box<dyn Error + Send + Sync>>>;
}
