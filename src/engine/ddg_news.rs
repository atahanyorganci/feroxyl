//! `DuckDuckGo` News search engine
//!
//! Port of `SearXNG`'s `duckduckgo_extra.py` with `ddg_category=news`.
//! Uses the JSON API at <https://duckduckgo.com/news.js>

use std::error::Error;

use reqwest::{
    Method,
    header::{HeaderName, HeaderValue},
};
use scraper::Html;
use serde::Deserialize;

use super::ddg::{extr, locale_to_ddg_region};
use crate::engine::{Safesearch, SearchParams, SearchResult};

const DDG_SEARCH_URL: &str = "https://duckduckgo.com/";
const DDG_NEWS_URL: &str = "https://duckduckgo.com/news.js";

/// Strips HTML tags from a string (e.g. excerpt) to plain text.
fn html_to_text(html: &str) -> String {
    let fragment = Html::parse_fragment(html);
    fragment
        .root_element()
        .text()
        .fold(String::new(), |mut acc, s| {
            acc.push_str(s);
            acc
        })
        .trim()
        .to_string()
}

/// Phase of the `DuckDuckGo` News state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DdgNewsPhase {
    /// Need to fetch VQD token (GET to duckduckgo.com)
    NeedVqd,
    /// Ready to search (GET to news.js)
    NeedSearch,
    /// No more requests
    Done,
}

/// Raw news result from `DuckDuckGo` JSON API.
#[derive(Debug, Deserialize)]
struct DdgNewsResult {
    url: String,
    title: String,
    #[serde(default)]
    excerpt: String,
    #[serde(default)]
    #[allow(dead_code)]
    source: String,
    #[serde(default)]
    #[allow(dead_code)]
    date: i64,
}

/// Raw response from `DuckDuckGo` news.js API.
#[derive(Debug, Deserialize)]
struct DdgNewsResponse {
    #[serde(default)]
    results: Vec<DdgNewsResult>,
}

/// Stateful `DuckDuckGo` News search provider implementing `SearchProvider`.
#[derive(Debug)]
pub struct DuckDuckGoNews {
    phase: DdgNewsPhase,
    vqd: Option<String>,
    results: Vec<SearchResult>,
}

impl Default for DuckDuckGoNews {
    fn default() -> Self {
        Self {
            phase: DdgNewsPhase::NeedVqd,
            vqd: None,
            results: Vec::with_capacity(32),
        }
    }
}

impl DuckDuckGoNews {
    fn build_vqd_request(
        params: &SearchParams,
    ) -> Result<reqwest::Request, Box<dyn Error + Send + Sync>> {
        let query_string = serde_urlencoded::to_string([("q", params.query.as_str())])
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        let url = format!("{DDG_SEARCH_URL}?{query_string}");
        let url = reqwest::Url::parse(&url).map_err(|e| std::io::Error::other(e.to_string()))?;
        let mut request = reqwest::Request::new(Method::GET, url);
        let headers = request.headers_mut();
        headers.insert(
            HeaderName::from_static("accept-language"),
            HeaderValue::from_static("en-US,en;q=0.9"),
        );
        headers.insert(
            HeaderName::from_static("user-agent"),
            HeaderValue::from_static(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
            ),
        );
        Ok(request)
    }

    fn build_news_request(
        &self,
        params: &SearchParams,
    ) -> Result<reqwest::Request, Box<dyn Error + Send + Sync>> {
        let region = locale_to_ddg_region(&params.locale);
        let vqd = self
            .vqd
            .as_deref()
            .ok_or("VQD required for DuckDuckGo News but could not be obtained")?;

        let mut query_pairs: Vec<(&str, &str)> = vec![
            ("q", params.query.as_str()),
            ("o", "json"),
            ("l", region.as_str()),
            ("f", ",,,,,,"),
            ("vqd", vqd),
        ];

        // Safesearch: Off=1, Moderate=None, Strict=1 (SearXNG: safesearch_args)
        match params.safesearch {
            Safesearch::Moderate => {}
            Safesearch::Off | Safesearch::Strict => query_pairs.push(("p", "1")),
        }

        let query_string = serde_urlencoded::to_string(&query_pairs)
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        let url = format!("{DDG_NEWS_URL}?{query_string}");
        let url = reqwest::Url::parse(&url).map_err(|e| std::io::Error::other(e.to_string()))?;

        let mut request = reqwest::Request::new(Method::GET, url);
        let headers = request.headers_mut();
        headers.insert(
            HeaderName::from_static("referer"),
            HeaderValue::from_static("https://duckduckgo.com/"),
        );
        headers.insert(
            HeaderName::from_static("x-requested-with"),
            HeaderValue::from_static("XMLHttpRequest"),
        );
        headers.insert(
            HeaderName::from_static("accept-language"),
            HeaderValue::from_static("en-US,en;q=0.9"),
        );
        headers.insert(
            HeaderName::from_static("user-agent"),
            HeaderValue::from_static(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
            ),
        );

        // Cookies: ad (lang), ah/l (region), p (safesearch)
        let ad = region.replace('-', "_");
        let mut cookie_parts = vec![
            format!("ad={ad}"),
            format!("ah={region}"),
            format!("l={region}"),
        ];
        match params.safesearch {
            Safesearch::Off => cookie_parts.push("p=-2".to_string()),
            Safesearch::Moderate => {}
            Safesearch::Strict => cookie_parts.push("p=1".to_string()),
        }
        let cookie_value = cookie_parts.join("; ");
        headers.insert(
            HeaderName::from_static("cookie"),
            HeaderValue::try_from(cookie_value)
                .map_err(|e| std::io::Error::other(e.to_string()))?,
        );

        Ok(request)
    }
}

impl crate::engine::SearchProvider for DuckDuckGoNews {
    fn name() -> &'static str {
        "ddg_news"
    }

    fn build_request(
        &mut self,
        params: &SearchParams,
    ) -> Result<reqwest::Request, Box<dyn Error + Send + Sync>> {
        if params.query.len() >= 500 {
            tracing::warn!(
                len = params.query.len(),
                "Query too long for DuckDuckGo News"
            );
            return Err("Query too long (max 499 characters)".into());
        }

        match self.phase {
            DdgNewsPhase::NeedVqd => {
                let req = Self::build_vqd_request(params)?;
                Ok(req)
            }
            DdgNewsPhase::NeedSearch => {
                let req = self.build_news_request(params)?;
                self.phase = DdgNewsPhase::Done;
                Ok(req)
            }
            DdgNewsPhase::Done => Err("No more requests".into()),
        }
    }

    fn parse_response(&mut self, body: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        if self.phase == DdgNewsPhase::NeedVqd {
            self.vqd = extr(body, "vqd=\"", "\"");
            if self.vqd.is_none() {
                tracing::debug!("VQD token not found in DuckDuckGo response");
            }
            self.phase = DdgNewsPhase::NeedSearch;
            return Ok(());
        }

        let response: DdgNewsResponse = serde_json::from_str(body)
            .map_err(|e| std::io::Error::other(format!("DuckDuckGo News JSON parse error: {e}")))?;

        for r in response.results {
            let content = if r.excerpt.is_empty() {
                None
            } else {
                Some(html_to_text(&r.excerpt))
            };
            self.results.push(SearchResult {
                title: r.title,
                url: r.url,
                content: content.filter(|s| !s.is_empty()),
            });
        }

        Ok(())
    }

    fn results(&mut self) -> Option<Result<Vec<SearchResult>, Box<dyn Error + Send + Sync>>> {
        if self.results.is_empty() {
            None
        } else {
            Some(Ok(std::mem::take(&mut self.results)))
        }
    }
}
