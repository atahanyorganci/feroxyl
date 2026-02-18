//! Google search engine
//!
//! Port of SearXNG's google.py engine. Uses async/arc HTML format.

use rand::Rng;
use reqwest::header::{HeaderName, HeaderValue};
use reqwest::Method;
use reqwest::Url;
use scraper::{ElementRef, Html, Selector};
use std::error::Error;
use std::time::{Duration, Instant};

use crate::engine::{SearchParams, SearchProvider, SearchResult};

/// Charset for arc_id random string (matches SearXNG: a-zA-Z0-9_-)
const ARC_ID_CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_-";

/// Time range to Google tbs (qdr:) code
fn time_range_to_google_tbs(tr: crate::engine::TimeRange) -> Option<&'static str> {
    match tr {
        crate::engine::TimeRange::Any => None,
        crate::engine::TimeRange::Day => Some("d"),
        crate::engine::TimeRange::Week => Some("w"),
        crate::engine::TimeRange::Month => Some("m"),
        crate::engine::TimeRange::Year => Some("y"),
    }
}

/// Safesearch to Google safe param
fn safesearch_to_google(s: crate::engine::Safesearch) -> &'static str {
    match s {
        crate::engine::Safesearch::Off => "off",
        crate::engine::Safesearch::Moderate => "medium",
        crate::engine::Safesearch::Strict => "high",
    }
}

fn build_google_search_url(
    params: &SearchParams,
    async_param: &str,
) -> Result<Url, Box<dyn Error>> {
    let start = 0u32;
    let mut url = Url::parse("https://www.google.com/search")?;
    {
        let mut pairs = url.query_pairs_mut();
        pairs
            .append_pair("q", &params.query)
            .append_pair("hl", "en-US")
            .append_pair("lr", "lang_en")
            .append_pair("cr", "countryUS")
            .append_pair("ie", "utf8")
            .append_pair("oe", "utf8")
            .append_pair("filter", "0")
            .append_pair("start", &start.to_string())
            .append_pair("asearch", "arc")
            .append_pair("async", async_param);
        if let Some(tbs) = time_range_to_google_tbs(params.time_range) {
            pairs.append_pair("tbs", &format!("qdr:{}", tbs));
        }
        if params.safesearch != crate::engine::Safesearch::Off {
            pairs.append_pair("safe", safesearch_to_google(params.safesearch));
        }
    }
    Ok(url)
}

fn extract_title(element: ElementRef) -> Result<String, Box<dyn Error>> {
    let selector = Selector::parse("div[role='link']").unwrap();
    if let Some(element) = element.select(&selector).next() {
        return Ok(element.text().collect::<String>());
    }
    let selector = Selector::parse("div[role*='link']").unwrap();
    if let Some(element) = element.select(&selector).next() {
        return Ok(element.text().collect::<String>());
    }
    let selector = Selector::parse("[data-snf='GuLy6c']").unwrap();
    if let Some(element) = element.select(&selector).next() {
        return Ok(element.text().collect::<String>());
    }
    Err("No title found".into())
}

fn extract_content(element: ElementRef) -> Option<String> {
    let select = Selector::parse("[data-sncf*='1']").unwrap();
    let mut content = String::with_capacity(1024);
    for element in element.select(&select) {
        let text = element.text();
        for text in text {
            content.push_str(text);
        }
    }
    if content.is_empty() {
        None
    } else {
        Some(content)
    }
}

fn extract_url(root: ElementRef) -> Result<String, Box<dyn Error>> {
    let link_selector = Selector::parse("a[href*='/url?q=']").unwrap();
    if let Some(link) = root.select(&link_selector).next() {
        let href = link.value().attr("href").unwrap();
        let url = format!("https://www.google.com{}", href);
        let url = Url::parse(&url).unwrap();
        let url = url
            .query_pairs()
            .find(|(key, _)| key == "q")
            .unwrap()
            .1
            .to_string();
        Ok(url)
    } else {
        Err("No link found".into())
    }
}

fn parse_google_result(element: ElementRef) -> Result<SearchResult, Box<dyn Error>> {
    let title = extract_title(element)?;
    let url = extract_url(element)?;
    let content = extract_content(element);
    Ok(SearchResult {
        title,
        url,
        content,
    })
}

/// Stateful Google search provider implementing SearchProvider.
#[derive(Debug)]
pub struct Google {
    results: Vec<SearchResult>,
    /// arc_id prefix, regenerated every hour (SearXNG behavior)
    arc_id_prefix: Option<String>,
    arc_id_created_at: Option<Instant>,
}

impl Default for Google {
    fn default() -> Self {
        Self::new()
    }
}

impl Google {
    pub fn new() -> Self {
        Self {
            results: Vec::with_capacity(32),
            arc_id_prefix: None,
            arc_id_created_at: None,
        }
    }

    /// Format of the async parameter for Google's arc UI.
    /// arc_id is randomly generated and cached for 1 hour on the provider.
    fn ui_async(&mut self, start: u32) -> String {
        let invalidate = self.arc_id_prefix.is_none()
            || self
                .arc_id_created_at
                .map(|t| t.elapsed() > Duration::from_secs(3600))
                .unwrap_or(true);

        if invalidate {
            let mut rng = rand::rng();
            self.arc_id_prefix = Some(
                (0..23)
                    .map(|_| ARC_ID_CHARSET[rng.random_range(0..ARC_ID_CHARSET.len())] as char)
                    .collect(),
            );
            self.arc_id_created_at = Some(Instant::now());
        }

        let prefix = self.arc_id_prefix.as_ref().unwrap();
        format!("arc_id:srp_{}_1{:02},use_ac:true,_fmt:prog", prefix, start)
    }
}

impl SearchProvider for Google {
    fn build_request(
        &mut self,
        params: Option<SearchParams>,
    ) -> Result<Option<reqwest::Request>, Box<dyn Error + Send + Sync>> {
        let params = match params {
            Some(p) => p,
            None => return Ok(None),
        };

        let start = 0u32;
        let async_param = self.ui_async(start);
        let url = build_google_search_url(&params, &async_param)
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        let mut request = reqwest::Request::new(Method::GET, url);
        let headers = request.headers_mut();
        headers.insert(
            HeaderName::from_static("accept"),
            HeaderValue::from_static("*/*"),
        );
        headers.insert(
            HeaderName::from_static("sec-fetch-dest"),
            HeaderValue::from_static("empty"),
        );
        headers.insert(
            HeaderName::from_static("sec-fetch-mode"),
            HeaderValue::from_static("cors"),
        );
        headers.insert(
            HeaderName::from_static("sec-fetch-site"),
            HeaderValue::from_static("same-origin"),
        );
        headers.insert(
            HeaderName::from_static("sec-fetch-user"),
            HeaderValue::from_static("?1"),
        );
        headers.insert(
            HeaderName::from_static("sec-gpc"),
            HeaderValue::from_static("1"),
        );
        headers.insert(
            HeaderName::from_static("user-agent"),
            HeaderValue::from_static(
                "Mozilla/5.0 (iPhone; CPU iPhone OS 18_6_2 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) GSA/399.2.845414227 Mobile/15E148 Safari/604.1",
            ),
        );
        headers.insert(
            HeaderName::from_static("cookie"),
            HeaderValue::from_static("CONSENT=YES+"),
        );
        Ok(Some(request))
    }

    fn parse_response(&mut self, body: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut html = body.to_string();
        let start_index = html.find("<div").ok_or("No <div> found")?;
        html = html[start_index..].to_string();
        let end_index = html.rfind("</div>").ok_or("No </div> found")?;
        html = html[..end_index].to_string();

        let document = Html::parse_fragment(&html);
        let selector = Selector::parse("div.MjjYud").unwrap();
        for result in document.select(&selector) {
            if let Ok(result) = parse_google_result(result) {
                self.results.push(result);
            }
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
