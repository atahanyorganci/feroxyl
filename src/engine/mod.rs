//! Search engine implementations
//!
//! Common parameters follow SearXNG's RequestParams / SearchQuery model.

use std::collections::HashMap;
use std::error::Error;
use std::time::Duration;

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

/// Ranked search result with a score.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RankedSearchResult {
    /// Search result title.
    pub title: String,
    /// Search result URL.
    pub url: String,
    /// Search result content.
    pub content: Option<String>,
    /// Search result positions in search engines.
    pub position: Vec<(&'static str, usize)>,
    /// Final score calculated from positions.
    pub score: f32,
}

/// Runs a search provider until completion, executing HTTP requests with the given client.
#[tracing::instrument(skip(params), fields(provider = P::name(), query = %params.query))]
pub async fn run_provider<P: SearchProvider>(
    params: &SearchParams,
) -> Result<Vec<SearchResult>, Box<dyn Error + Send + Sync>> {
    let mut provider = P::default();
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(5))
        .build()?;

    loop {
        tracing::debug!(provider = P::name(), "Building request");
        let request = provider.build_request(params)?;
        let response = client.execute(request).await?;
        let status = response.status();
        let body = response.text().await?;
        tracing::debug!(status = %status, body_len = body.len(), "Received response");
        provider.parse_response(&body)?;

        match provider.results() {
            Some(Ok(results)) => {
                tracing::info!(count = results.len(), "Provider completed");
                break Ok(results);
            }
            Some(Err(e)) => {
                tracing::error!(error = %e, "Provider failed");
                return Err(e);
            }
            None => {}
        }
    }
}

/// Normalizes a URL for deduplication (SearXNG-style: host + path, case-insensitive).
fn normalize_url_key(url_str: &str) -> Option<String> {
    let parsed = reqwest::Url::parse(url_str).ok()?;
    let host = parsed.host_str()?.to_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(&host);
    Some(format!("{}|{}", host, parsed.path()))
}

/// Merged result entry: (result, engine positions).
type MergedEntry = (SearchResult, Vec<(&'static str, usize)>);

/// Calculates score from positions (SearXNG results.py: weight=1.0, score = sum(weight/position)).
fn calculate_score(positions: &[usize]) -> f32 {
    let weight = positions.len() as f32;
    positions.iter().map(|&p| weight / (p as f32)).sum()
}

#[tracing::instrument(skip(params), fields(query = %params.query))]
pub async fn run_meta_search(
    params: &SearchParams,
) -> Result<Vec<RankedSearchResult>, Box<dyn Error + Send + Sync>> {
    tracing::debug!("Starting parallel provider queries");
    let (ddg_res, google_res) = tokio::join!(
        run_provider::<ddg::DuckDuckGo>(params),
        run_provider::<google::Google>(params),
    );

    let ddg_results = ddg_res.unwrap_or_else(|e| {
        tracing::warn!(error = %e, provider = "ddg", "Provider failed, using empty results");
        Vec::new()
    });
    let google_results = google_res.unwrap_or_else(|e| {
        tracing::warn!(error = %e, provider = "google", "Provider failed, using empty results");
        Vec::new()
    });

    tracing::debug!(
        ddg_count = ddg_results.len(),
        google_count = google_results.len(),
        "Provider results received"
    );

    let mut merged: HashMap<String, MergedEntry> = HashMap::new();

    for (pos, r) in ddg_results.into_iter().enumerate() {
        let key = normalize_url_key(&r.url).unwrap_or_else(|| r.url.clone());
        let engine_name = ddg::DuckDuckGo::name();
        merged
            .entry(key)
            .and_modify(|(existing, positions)| {
                positions.push((engine_name, pos + 1));
                if r.content.as_ref().map_or(0, |c| c.len())
                    > existing.content.as_ref().map_or(0, |c| c.len())
                {
                    existing.content = r.content.clone();
                }
                if r.title.len() > existing.title.len() {
                    existing.title = r.title.clone();
                }
            })
            .or_insert_with(|| (r, vec![(engine_name, pos + 1)]));
    }

    for (pos, r) in google_results.into_iter().enumerate() {
        let key = normalize_url_key(&r.url).unwrap_or_else(|| r.url.clone());
        let engine_name = google::Google::name();
        merged
            .entry(key)
            .and_modify(|(existing, positions)| {
                positions.push((engine_name, pos + 1));
                if r.content.as_ref().map_or(0, |c| c.len())
                    > existing.content.as_ref().map_or(0, |c| c.len())
                {
                    existing.content = r.content.clone();
                }
                if r.title.len() > existing.title.len() {
                    existing.title = r.title.clone();
                }
            })
            .or_insert_with(|| (r, vec![(engine_name, pos + 1)]));
    }

    let mut ranked: Vec<RankedSearchResult> = merged
        .into_values()
        .map(|(r, positions)| {
            let positions_only: Vec<usize> = positions.iter().map(|(_, p)| *p).collect();
            RankedSearchResult {
                title: r.title,
                url: r.url,
                content: r.content,
                position: positions,
                score: calculate_score(&positions_only),
            }
        })
        .collect();

    ranked.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    tracing::info!(count = ranked.len(), "Meta search completed");
    Ok(ranked)
}

/// Trait for search providers as a state machine. HTTP execution is handled externally.
pub trait SearchProvider
where
    Self: Default,
{
    fn name() -> &'static str;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_url_key() {
        assert_eq!(
            normalize_url_key("https://example.com/path"),
            Some("example.com|/path".to_string())
        );
        assert_eq!(
            normalize_url_key("https://www.example.com/path"),
            Some("example.com|/path".to_string())
        );
        assert_eq!(
            normalize_url_key("http://EXAMPLE.COM/Path"),
            Some("example.com|/Path".to_string())
        );
        assert_eq!(normalize_url_key("not-a-url"), None);
    }

    #[test]
    fn test_calculate_score() {
        // Single engine, position 1: weight=1, score=1/1=1
        assert!((calculate_score(&[1]) - 1.0).abs() < 1e-6);
        // Two engines, positions 1 and 2: weight=2, score=2/1+2/2=3
        assert!((calculate_score(&[1, 2]) - 3.0).abs() < 1e-6);
        // Two engines, positions 1 and 3: weight=2, score=2/1+2/3≈2.667
        assert!((calculate_score(&[1, 3]) - 2.666_667).abs() < 1e-5);
    }
}
