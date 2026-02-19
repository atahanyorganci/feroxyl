//! `DuckDuckGo` search engines (web, news).
//!
//! Shared utilities and re-exports for `DuckDuckGo` implementations.

mod news;
mod web;

pub use news::DuckDuckGoNews;
use reqwest::{
    Method,
    header::{HeaderName, HeaderValue},
};
pub use web::DuckDuckGo;

use crate::engine::{Locale, SearchParams};

const DDG_SEARCH_URL: &str = "https://duckduckgo.com/";

/// `DuckDuckGo` region code from Locale: All -> "wt-wt"; otherwise lowercased with hyphens.
pub(crate) fn locale_to_ddg_region(locale: &Locale) -> String {
    match locale {
        Locale::All => "wt-wt".to_string(),
        Locale::EnUS => "en-us".to_string(),
        Locale::EnGB => "en-gb".to_string(),
        Locale::TrTR => "tr-tr".to_string(),
        Locale::Other(s) => s.to_lowercase().replace('_', "-"),
    }
}

/// Extracts text between `begin` and `end` in `txt`
pub(crate) fn extr(txt: &str, begin: &str, end: &str) -> Option<String> {
    let start_idx = txt.find(begin)?;
    let after_begin = start_idx + begin.len();
    let end_idx = txt[after_begin..].find(end)?;
    Some(txt[after_begin..after_begin + end_idx].to_string())
}

/// Builds the VQD token request (GET to duckduckgo.com). Both web and news search
/// require fetching a VQD token before the actual search request.
pub(crate) fn build_vqd_request(
    params: &SearchParams,
) -> Result<reqwest::Request, Box<dyn std::error::Error + Send + Sync>> {
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
