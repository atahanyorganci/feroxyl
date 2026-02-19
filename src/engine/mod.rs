//! Search engine implementations
//!
//! Common parameters follow `SearXNG`'s `RequestParams` / `SearchQuery` model.

use std::{
    collections::HashMap,
    error::Error,
    time::{Duration, Instant},
};

mod bing;
mod bing_images;
mod brave;
mod ddg;
mod google;
mod google_images;
mod startpage;
mod startpage_images;

pub use bing::Bing;
pub use bing_images::BingImages;
pub use brave::Brave;
pub use ddg::DuckDuckGo;
pub use google::Google;
pub use google_images::GoogleImages;
pub use startpage::Startpage;
pub use startpage_images::StartpageImages;
use tokio::task::JoinSet;

/// Unified search result type for all providers
#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub content: Option<String>,
}

/// Image search result type for image providers.
/// Mirrors `SearXNG`'s images.html template schema used by `bing_images`, `google_images`,
/// `duckduckgo_extra`, brave, etc.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ImageResult {
    /// URL to the source page where the image is hosted.
    pub url: String,
    /// Full-size image URL (the actual image to display/download).
    pub img_src: String,
    /// Thumbnail URL for grid display. Falls back to `img_src` when absent.
    pub thumbnail_src: Option<String>,
    /// Image title or caption.
    pub title: String,
    /// Optional description or alt text.
    pub content: Option<String>,
    /// Source name (e.g. site name, domain).
    pub source: Option<String>,
    /// Resolution string (e.g. "1920 x 1080").
    pub resolution: Option<String>,
    /// Image format (e.g. "PNG", "JPEG").
    pub img_format: Option<String>,
    /// File size string (e.g. "1.2 MB").
    pub filesize: Option<String>,
    /// Creator or author.
    pub author: Option<String>,
}

/// Time range filter for search results (`SearXNG`: `time_range`).
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

/// Safe search filter level (`SearXNG`: safesearch 0/1/2).
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
/// Mirrors `SearXNG`'s `searxng_locale`.
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
    #[must_use]
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
/// Mirrors `SearXNG`'s `RequestParams`: query, safesearch, `time_range`, `searxng_locale`.
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
    pub score: f64,
}

/// Ranked image result with score from meta-search merge.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RankedImageResult {
    /// URL to the source page where the image is hosted.
    pub url: String,
    /// Full-size image URL (the actual image to display/download).
    pub img_src: String,
    /// Thumbnail URL for grid display. Falls back to `img_src` when absent.
    pub thumbnail_src: Option<String>,
    /// Image title or caption.
    pub title: String,
    /// Optional description or alt text.
    pub content: Option<String>,
    /// Source name (e.g. site name, domain).
    pub source: Option<String>,
    /// Resolution string (e.g. "1920 x 1080").
    pub resolution: Option<String>,
    /// Image format (e.g. "PNG", "JPEG").
    pub img_format: Option<String>,
    /// File size string (e.g. "1.2 MB").
    pub filesize: Option<String>,
    /// Creator or author.
    pub author: Option<String>,
    /// Provider positions (engine name, rank).
    pub position: Vec<(String, usize)>,
    /// Final score calculated from positions.
    pub score: f64,
}

/// Runs a search provider until completion, executing HTTP requests with the given client.
///
/// # Errors
///
/// Returns an error if the HTTP request fails, response parsing fails, or the provider returns an error.
#[tracing::instrument(skip(params), fields(provider = P::name(), query = %params.query))]
pub async fn run_provider<P: SearchProvider>(
    params: &SearchParams,
) -> Result<Vec<SearchResult>, Box<dyn Error + Send + Sync>> {
    let start = Instant::now();
    let mut provider = P::default();
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(5))
        .cookie_store(true)
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
                let elapsed = start.elapsed();
                tracing::info!(
                    provider = P::name(),
                    count = results.len(),
                    elapsed_ms = elapsed.as_millis(),
                    "Provider completed"
                );
                break Ok(results);
            }
            Some(Err(e)) => {
                let elapsed = start.elapsed();
                tracing::error!(
                    provider = P::name(),
                    error = %e,
                    elapsed_ms = elapsed.as_millis(),
                    "Provider failed"
                );
                return Err(e);
            }
            None => {}
        }
    }
}

/// Runs multiple search providers in parallel and merges results by URL with ranking.
///
/// # Errors
///
/// Does not fail overall; individual provider failures are logged and skipped.
#[tracing::instrument(skip(params), fields(query = %params.query))]
pub async fn run_meta_search(
    providers: &[Provider],
    params: &SearchParams,
) -> Result<Vec<RankedSearchResult>, Box<dyn Error + Send + Sync>> {
    let start = Instant::now();
    tracing::debug!("Starting parallel provider queries");
    let mut results_set = JoinSet::new();
    for provider in providers {
        let name = provider.name();
        tracing::debug!(provider = name, "Spawning provider");
        let provider = *provider;
        let params = params.clone();
        results_set
            .spawn(async move { provider.run(&params).await.map(|results| (name, results)) });
    }

    let mut merged: HashMap<String, RankedSearchResult> = HashMap::new();

    while let Some(join_result) = results_set.join_next().await {
        let (engine_name, results) = match join_result {
            Ok(Ok((name, results))) => {
                tracing::debug!(provider = name, count = results.len(), "Provider completed");
                (name, results)
            }
            Ok(Err(e)) => {
                tracing::warn!(error = %e, "Provider failed");
                continue;
            }
            Err(e) => {
                if e.is_cancelled() {
                    break;
                }
                tracing::warn!(error = %e, "Provider failed");
                continue;
            }
        };
        for (pos, r) in results.into_iter().enumerate() {
            merged
                .entry(r.url.clone())
                .and_modify(|existing| {
                    existing.position.push((engine_name, pos + 1));
                    let rank = u32::try_from(pos + 1).unwrap_or(u32::MAX);
                    existing.score += 1.0 / f64::from(rank);
                })
                .or_insert_with(|| {
                    let rank = u32::try_from(pos + 1).unwrap_or(u32::MAX);
                    RankedSearchResult {
                        title: r.title,
                        url: r.url,
                        content: r.content,
                        position: vec![(engine_name, pos + 1)],
                        score: 1.0 / f64::from(rank),
                    }
                });
        }
    }

    let mut ranked: Vec<RankedSearchResult> = merged.into_values().collect();
    ranked.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let elapsed = start.elapsed();
    tracing::info!(
        count = ranked.len(),
        elapsed_ms = elapsed.as_millis(),
        "Meta search completed"
    );
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
    ///
    /// # Errors
    ///
    /// Returns an error if URL construction or request building fails.
    fn build_request(
        &mut self,
        params: &SearchParams,
    ) -> Result<reqwest::Request, Box<dyn Error + Send + Sync>>;

    /// Parse an HTTP response body. Updates internal state (e.g. result queue).
    ///
    /// # Errors
    ///
    /// Returns an error if response parsing fails.
    fn parse_response(&mut self, body: &str) -> Result<(), Box<dyn Error + Send + Sync>>;

    /// Yield the next result. None when no more results; caller loops back to `build_request`.
    fn results(&mut self) -> Option<Result<Vec<SearchResult>, Box<dyn Error + Send + Sync>>>;
}

/// Trait for image search providers. Same state-machine flow as `SearchProvider` but yields `ImageResult`.
pub trait ImageSearchProvider
where
    Self: Default,
{
    fn name() -> &'static str;

    /// # Errors
    ///
    /// Returns an error if URL construction or request building fails.
    fn build_request(
        &mut self,
        params: &SearchParams,
    ) -> Result<reqwest::Request, Box<dyn Error + Send + Sync>>;

    /// # Errors
    ///
    /// Returns an error if response parsing fails.
    fn parse_response(&mut self, body: &str) -> Result<(), Box<dyn Error + Send + Sync>>;

    fn results(&mut self) -> Option<Result<Vec<ImageResult>, Box<dyn Error + Send + Sync>>>;
}

/// Runs an image search provider until completion.
///
/// # Errors
///
/// Returns an error if the HTTP request fails, response parsing fails, or the provider returns an error.
#[tracing::instrument(skip(params), fields(provider = P::name(), query = %params.query))]
pub async fn run_image_provider<P: ImageSearchProvider>(
    params: &SearchParams,
) -> Result<Vec<ImageResult>, Box<dyn Error + Send + Sync>> {
    let start = Instant::now();
    let mut provider = P::default();
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(5))
        .cookie_store(true)
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
                let elapsed = start.elapsed();
                tracing::info!(
                    provider = P::name(),
                    count = results.len(),
                    elapsed_ms = elapsed.as_millis(),
                    "Image provider completed"
                );
                break Ok(results);
            }
            Some(Err(e)) => {
                let elapsed = start.elapsed();
                tracing::error!(
                    provider = P::name(),
                    error = %e,
                    elapsed_ms = elapsed.as_millis(),
                    "Image provider failed"
                );
                return Err(e);
            }
            None => {}
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Provider {
    DuckDuckGo,
    Google,
    Brave,
    Startpage,
    Bing,
}

/// Error when parsing an invalid provider string.
#[derive(Debug, thiserror::Error)]
#[error("invalid provider: {0}")]
pub struct InvalidProvider(pub String);

impl std::str::FromStr for Provider {
    type Err = InvalidProvider;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "duckduckgo" | "ddg" => Ok(Provider::DuckDuckGo),
            "google" => Ok(Provider::Google),
            "brave" => Ok(Provider::Brave),
            "startpage" => Ok(Provider::Startpage),
            "bing" => Ok(Provider::Bing),
            other => Err(InvalidProvider(other.to_string())),
        }
    }
}

impl<'de> serde::Deserialize<'de> for Provider {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

impl Provider {
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Provider::DuckDuckGo => DuckDuckGo::name(),
            Provider::Google => Google::name(),
            Provider::Brave => Brave::name(),
            Provider::Startpage => Startpage::name(),
            Provider::Bing => Bing::name(),
        }
    }

    /// # Errors
    ///
    /// Returns an error if the provider fails (HTTP, parsing, or provider error).
    pub async fn run(
        &self,
        params: &SearchParams,
    ) -> Result<Vec<SearchResult>, Box<dyn Error + Send + Sync>> {
        match self {
            Provider::DuckDuckGo => run_provider::<DuckDuckGo>(params).await,
            Provider::Google => run_provider::<Google>(params).await,
            Provider::Brave => run_provider::<Brave>(params).await,
            Provider::Startpage => run_provider::<Startpage>(params).await,
            Provider::Bing => run_provider::<Bing>(params).await,
        }
    }
}

/// Image search provider enum. Mirrors `Provider` for web search.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageProvider {
    BingImages,
    GoogleImages,
    StartpageImages,
}

/// Error when parsing an invalid image provider string.
#[derive(Debug, thiserror::Error)]
#[error("invalid image provider: {0}")]
pub struct InvalidImageProvider(pub String);

impl std::str::FromStr for ImageProvider {
    type Err = InvalidImageProvider;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "bing_images" | "bing" => Ok(ImageProvider::BingImages),
            "google_images" | "google" => Ok(ImageProvider::GoogleImages),
            "startpage_images" | "startpage" => Ok(ImageProvider::StartpageImages),
            other => Err(InvalidImageProvider(other.to_string())),
        }
    }
}

impl<'de> serde::Deserialize<'de> for ImageProvider {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

impl ImageProvider {
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            ImageProvider::BingImages => BingImages::name(),
            ImageProvider::GoogleImages => GoogleImages::name(),
            ImageProvider::StartpageImages => StartpageImages::name(),
        }
    }

    /// # Errors
    ///
    /// Returns an error if the provider fails (HTTP, parsing, or provider error).
    pub async fn run(
        &self,
        params: &SearchParams,
    ) -> Result<Vec<ImageResult>, Box<dyn Error + Send + Sync>> {
        match self {
            ImageProvider::BingImages => run_image_provider::<BingImages>(params).await,
            ImageProvider::GoogleImages => run_image_provider::<GoogleImages>(params).await,
            ImageProvider::StartpageImages => run_image_provider::<StartpageImages>(params).await,
        }
    }
}

/// Runs multiple image providers in parallel and merges results by URL with ranking.
///
/// # Errors
///
/// Does not fail overall; individual provider failures are logged and skipped.
#[tracing::instrument(skip(params), fields(query = %params.query))]
pub async fn run_meta_image_search(
    providers: &[ImageProvider],
    params: &SearchParams,
) -> Result<Vec<RankedImageResult>, Box<dyn Error + Send + Sync>> {
    let start = Instant::now();
    tracing::debug!("Starting parallel image provider queries");
    let mut results_set = JoinSet::new();
    for provider in providers {
        let name = provider.name();
        tracing::debug!(provider = name, "Spawning image provider");
        let provider = *provider;
        let params = params.clone();
        results_set
            .spawn(async move { provider.run(&params).await.map(|results| (name, results)) });
    }

    let mut merged: HashMap<String, RankedImageResult> = HashMap::new();

    while let Some(join_result) = results_set.join_next().await {
        let (engine_name, results) = match join_result {
            Ok(Ok((name, results))) => {
                tracing::debug!(provider = name, count = results.len(), "Image provider completed");
                (name, results)
            }
            Ok(Err(e)) => {
                tracing::warn!(error = %e, "Image provider failed");
                continue;
            }
            Err(e) => {
                if e.is_cancelled() {
                    break;
                }
                tracing::warn!(error = %e, "Image provider failed");
                continue;
            }
        };
        for (pos, r) in results.into_iter().enumerate() {
            let rank = u32::try_from(pos + 1).unwrap_or(u32::MAX);
            let score = 1.0 / f64::from(rank);
            merged
                .entry(r.url.clone())
                .and_modify(|existing| {
                    existing.position.push((engine_name.to_string(), pos + 1));
                    existing.score += score;
                })
                .or_insert_with(|| RankedImageResult {
                    url: r.url,
                    img_src: r.img_src,
                    thumbnail_src: r.thumbnail_src,
                    title: r.title,
                    content: r.content,
                    source: r.source,
                    resolution: r.resolution,
                    img_format: r.img_format,
                    filesize: r.filesize,
                    author: r.author,
                    position: vec![(engine_name.to_string(), pos + 1)],
                    score,
                });
        }
    }

    let mut ranked: Vec<RankedImageResult> = merged.into_values().collect();
    ranked.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let elapsed = start.elapsed();
    tracing::info!(
        count = ranked.len(),
        elapsed_ms = elapsed.as_millis(),
        "Meta image search completed"
    );
    Ok(ranked)
}
