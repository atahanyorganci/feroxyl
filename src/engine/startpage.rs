//! Startpage search engine
//!
//! Port of `SearXNG`'s startpage.py engine. Supports web search (category "web").
//! Uses HTML scraping and requires fetching an sc-code from the homepage first
//! to avoid bot detection. POSTs to <https://www.startpage.com/sp/search>

use reqwest::Method;
use reqwest::Url;
use reqwest::header::{HeaderName, HeaderValue};
use scraper::{Html, Selector};
use serde::Deserialize;
use std::collections::HashMap;
use std::error::Error;

use crate::engine::{Locale, Safesearch, SearchParams, SearchProvider, SearchResult, TimeRange};

const BASE_URL: &str = "https://www.startpage.com";
const BASE_URL_SLASH: &str = "https://www.startpage.com/";
const SEARCH_URL: &str = "https://www.startpage.com/sp/search";

/// Startpage-specific errors.
#[derive(Debug, thiserror::Error)]
pub enum StartpageError {
    #[error("Startpage CAPTCHA detected")]
    Captcha,

    #[error("failed to extract sc-code from homepage")]
    ScCodeNotFound,

    #[error("failed to parse search results: {0}")]
    ParseError(String),
}

/// Time range to Startpage `with_date` param (d, w, m, y).
fn time_range_to_startpage(tr: TimeRange) -> &'static str {
    match tr {
        TimeRange::Any => "",
        TimeRange::Day => "d",
        TimeRange::Week => "w",
        TimeRange::Month => "m",
        TimeRange::Year => "y",
    }
}

/// Safesearch to Startpage `disable_family_filter` cookie (0=strict, 1=off).
fn safesearch_to_disable_family_filter(s: Safesearch) -> &'static str {
    match s {
        Safesearch::Off => "1",
        Safesearch::Moderate | Safesearch::Strict => "0",
    }
}

/// Map Locale to Startpage language code (for language/lui form args and cookie).
fn locale_to_language(locale: &Locale) -> Option<&'static str> {
    match locale {
        Locale::All => None,
        Locale::EnUS | Locale::Other(_) => Some("english"),
        Locale::EnGB => Some("english_uk"),
        Locale::TrTR => Some("turkish"),
    }
}

/// Map Locale to Startpage region code (for `search_results_region` cookie).
fn locale_to_region(locale: &Locale) -> Option<&'static str> {
    match locale {
        Locale::EnUS => Some("en-US"),
        Locale::EnGB => Some("en_GB"),
        Locale::TrTR => Some("tr_TR"),
        Locale::All | Locale::Other(_) => None,
    }
}

/// Extract string between `begin` and the last occurrence of `end` in `txt`.
/// Uses last occurrence because the JSON may contain "}})" in nested structures.
fn extr(txt: &str, begin: &str, end: &str) -> Option<String> {
    let start_idx = txt.find(begin)?;
    let after_begin = start_idx + begin.len();
    let rest = &txt[after_begin..];
    let end_idx = rest.rfind(end)?;
    Some(rest[..end_idx].to_string())
}

/// Strip HTML tags from a string to get plain text.
fn html_to_text(html: &str) -> String {
    let fragment = Html::parse_fragment(html);
    fragment
        .root_element()
        .text()
        .collect::<String>()
        .trim()
        .to_string()
}

/// Parse published date prefix from content (e.g. "2 Sep 2014 ... " or "5 days ago ... ").
/// Returns the content with the date prefix stripped.
fn parse_published_date_prefix(content: &str) -> String {
    // Pattern: "2 Sep 2014 ... " or "31 Dec 2023 ... "
    let date_suffix = " ... ";
    if let Some(pos) = content.find(date_suffix) {
        let prefix = &content[..pos];
        // Check if prefix matches "DD Mon YYYY" (e.g. 2 Sep 2014)
        let parts: Vec<&str> = prefix.split_whitespace().collect();
        if parts.len() == 3
            && parts[0].len() <= 2
            && parts[0].chars().all(|c| c.is_ascii_digit())
            && parts[1].len() == 3
            && parts[2].len() == 4
            && parts[2].chars().all(|c| c.is_ascii_digit())
        {
            return content[pos + date_suffix.len()..].trim_start().to_string();
        }
        // Pattern: "5 days ago ... " or "1 day ago ... " (same " ... " suffix)
        if (prefix.ends_with(" days ago") || prefix.ends_with(" day ago"))
            && prefix
                .split_whitespace()
                .next()
                .is_some_and(|s| s.chars().all(|c| c.is_ascii_digit()))
        {
            return content[pos + date_suffix.len()..].trim_start().to_string();
        }
    }
    content.to_string()
}

/// Phase of the Startpage state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StartpagePhase {
    /// Need to fetch sc-code from homepage (GET).
    NeedScCode,
    /// Ready to search (POST).
    NeedSearch,
    /// No more requests.
    Done,
}

/// JSON structure for Startpage search response (extracted from React props).
#[derive(Debug, Deserialize)]
struct StartpageResponse {
    #[serde(default)]
    render: StartpageRender,
}

#[derive(Debug, Deserialize, Default)]
struct StartpageRender {
    #[serde(default)]
    presenter: StartpagePresenter,
}

#[derive(Debug, Deserialize, Default)]
struct StartpagePresenter {
    #[serde(default)]
    regions: StartpageRegions,
}

#[derive(Debug, Deserialize, Default)]
struct StartpageRegions {
    #[serde(default)]
    mainline: Vec<StartpageMainlineItem>,
}

#[derive(Debug, Deserialize)]
struct StartpageMainlineItem {
    #[serde(default)]
    display_type: String,
    #[serde(default)]
    results: Vec<serde_json::Value>,
}

/// Parse web result from JSON item.
fn get_web_result(item: &serde_json::Value) -> Option<SearchResult> {
    let click_url = item.get("clickUrl")?.as_str()?;
    let title = html_to_text(item.get("title")?.as_str().unwrap_or(""));
    let description = item
        .get("description")
        .and_then(|v: &serde_json::Value| v.as_str())
        .unwrap_or("");
    let mut content = html_to_text(description);
    content = parse_published_date_prefix(&content);

    if title.is_empty() {
        return None;
    }

    Some(SearchResult {
        title,
        url: click_url.to_string(),
        content: if content.is_empty() {
            None
        } else {
            Some(content)
        },
    })
}

/// Parse search response by extracting JSON from React.createElement and parsing results.
fn parse_search_response(body: &str) -> Result<Vec<SearchResult>, Box<dyn Error + Send + Sync>> {
    if body.contains("/sp/captcha") {
        return Err(StartpageError::Captcha.into());
    }

    let json_str = extr(body, "React.createElement(UIStartpage.AppSerpWeb, {", "}})")
        .ok_or_else(|| StartpageError::ParseError("could not extract JSON from response".into()))?;

    // Extracted content is the props object body; wrap with { } to form valid JSON.
    // Need two closing braces: content doesn't end with }; add }} to close.
    let json_str = format!("{{{json_str}}}}}");
    let parsed: StartpageResponse = serde_json::from_str(&json_str)
        .map_err(|e: serde_json::Error| StartpageError::ParseError(e.to_string()))?;

    let mut results = Vec::new();
    for mainline_item in parsed.render.presenter.regions.mainline {
        if mainline_item.display_type == "web-google" {
            for item in mainline_item.results {
                if let Some(r) = get_web_result(&item) {
                    results.push(r);
                }
            }
        }
    }

    Ok(results)
}

/// Extract sc-code from Startpage homepage HTML.
fn extract_sc_code(html: &str) -> Result<String, Box<dyn Error + Send + Sync>> {
    if html.contains("/sp/captcha") {
        return Err(StartpageError::Captcha.into());
    }

    let doc = Html::parse_document(html);
    let selector = Selector::parse("form#search input[name=\"sc\"]")
        .map_err(|e| StartpageError::ParseError(format!("invalid selector: {e}")))?;

    let input = doc
        .select(&selector)
        .next()
        .ok_or(StartpageError::ScCodeNotFound)?;

    let value = input
        .value()
        .attr("value")
        .ok_or(StartpageError::ScCodeNotFound)?;

    Ok(value.to_string())
}

/// Build preferences cookie value (N1N-joined keyEEEvalue pairs).
fn build_preferences_cookie(
    safesearch: Safesearch,
    language: Option<&str>,
    region: Option<&str>,
) -> String {
    let mut cookie: Vec<(&str, &str)> = vec![
        ("date_time", "world"),
        (
            "disable_family_filter",
            safesearch_to_disable_family_filter(safesearch),
        ),
        ("disable_open_in_new_window", "0"),
        ("enable_post_method", "1"),
        ("enable_proxy_safety_suggest", "1"),
        ("enable_stay_control", "1"),
        ("instant_answers", "1"),
        ("lang_homepage", "s/device/en/"),
        ("num_of_results", "10"),
        ("suggestions", "1"),
        ("wt_unit", "celsius"),
    ];

    if let Some(lang) = language {
        cookie.push(("language", lang));
        cookie.push(("language_ui", lang));
    }
    if let Some(reg) = region {
        cookie.push(("search_results_region", reg));
    }

    cookie
        .iter()
        .map(|(k, v)| format!("{k}EEE{v}"))
        .collect::<Vec<_>>()
        .join("N1N")
}

/// Stateful Startpage search provider implementing `SearchProvider`.
#[derive(Debug)]
pub struct Startpage {
    phase: StartpagePhase,
    sc_code: Option<String>,
    results: Vec<SearchResult>,
}

impl Default for Startpage {
    fn default() -> Self {
        Self {
            phase: StartpagePhase::NeedScCode,
            sc_code: None,
            results: Vec::with_capacity(32),
        }
    }
}

impl Startpage {
    fn build_sc_code_request(
        _params: &SearchParams,
    ) -> Result<reqwest::Request, Box<dyn Error + Send + Sync>> {
        let url = Url::parse(&format!("{BASE_URL}/"))
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        let mut request = reqwest::Request::new(Method::GET, url);

        let headers = request.headers_mut();
        headers.insert(
            HeaderName::from_static("accept"),
            HeaderValue::from_static(
                "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            ),
        );
        headers.insert(
            HeaderName::from_static("accept-language"),
            HeaderValue::from_static("en-US,en;q=0.5"),
        );
        headers.insert(
            HeaderName::from_static("user-agent"),
            HeaderValue::from_static(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
            ),
        );

        Ok(request)
    }

    fn build_search_request(
        &self,
        params: &SearchParams,
    ) -> Result<reqwest::Request, Box<dyn Error + Send + Sync>> {
        let sc_code = self
            .sc_code
            .as_ref()
            .ok_or(StartpageError::ScCodeNotFound)?;

        let language = locale_to_language(&params.locale);
        let region = locale_to_region(&params.locale);

        let mut form_data = HashMap::new();
        form_data.insert("query", params.query.clone());
        form_data.insert("cat", "web".to_string());
        form_data.insert("t", "device".to_string());
        form_data.insert("sc", sc_code.clone());
        form_data.insert(
            "with_date",
            time_range_to_startpage(params.time_range).to_string(),
        );
        form_data.insert("abp", "1".to_string());
        form_data.insert("abd", "1".to_string());
        form_data.insert("abe", "1".to_string());

        if let Some(lang) = language {
            form_data.insert("language", lang.to_string());
            form_data.insert("lui", lang.to_string());
        }

        let body = serde_urlencoded::to_string(&form_data)
            .map_err(|e| std::io::Error::other(e.to_string()))?;

        let url = Url::parse(SEARCH_URL).map_err(|e| std::io::Error::other(e.to_string()))?;
        let mut request = reqwest::Request::new(Method::POST, url);

        let headers = request.headers_mut();
        headers.insert(
            HeaderName::from_static("content-type"),
            HeaderValue::from_static("application/x-www-form-urlencoded"),
        );
        headers.insert(
            HeaderName::from_static("origin"),
            HeaderValue::from_static(BASE_URL),
        );
        headers.insert(
            HeaderName::from_static("referer"),
            HeaderValue::from_static(BASE_URL_SLASH),
        );
        headers.insert(
            HeaderName::from_static("accept"),
            HeaderValue::from_static(
                "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            ),
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

        let cookie_value = format!(
            "preferences={}",
            build_preferences_cookie(params.safesearch, language, region)
        );
        headers.insert(
            HeaderName::from_static("cookie"),
            HeaderValue::try_from(cookie_value)
                .map_err(|e| std::io::Error::other(e.to_string()))?,
        );

        *request.body_mut() = Some(reqwest::Body::from(body));

        Ok(request)
    }
}

impl SearchProvider for Startpage {
    fn name() -> &'static str {
        "startpage"
    }

    fn build_request(
        &mut self,
        params: &SearchParams,
    ) -> Result<reqwest::Request, Box<dyn Error + Send + Sync>> {
        match self.phase {
            StartpagePhase::NeedScCode => Self::build_sc_code_request(params),
            StartpagePhase::NeedSearch => {
                let req = self.build_search_request(params)?;
                self.phase = StartpagePhase::Done;
                Ok(req)
            }
            StartpagePhase::Done => Err("No more requests".into()),
        }
    }

    fn parse_response(&mut self, body: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        if self.phase == StartpagePhase::NeedScCode {
            self.sc_code = Some(extract_sc_code(body)?);
            self.phase = StartpagePhase::NeedSearch;
            return Ok(());
        }

        match parse_search_response(body) {
            Ok(results) => {
                self.results = results;
                Ok(())
            }
            Err(e) if e.to_string().contains("CAPTCHA") => Ok(()),
            Err(e) => Err(e),
        }
    }

    fn results(&mut self) -> Option<Result<Vec<SearchResult>, Box<dyn Error + Send + Sync>>> {
        if self.results.is_empty() {
            None
        } else {
            Some(Ok(std::mem::take(&mut self.results)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_range_to_startpage() {
        assert_eq!(time_range_to_startpage(TimeRange::Any), "");
        assert_eq!(time_range_to_startpage(TimeRange::Day), "d");
        assert_eq!(time_range_to_startpage(TimeRange::Week), "w");
        assert_eq!(time_range_to_startpage(TimeRange::Month), "m");
        assert_eq!(time_range_to_startpage(TimeRange::Year), "y");
    }

    #[test]
    fn test_safesearch_to_disable_family_filter() {
        assert_eq!(safesearch_to_disable_family_filter(Safesearch::Off), "1");
        assert_eq!(
            safesearch_to_disable_family_filter(Safesearch::Moderate),
            "0"
        );
        assert_eq!(safesearch_to_disable_family_filter(Safesearch::Strict), "0");
    }

    #[test]
    fn test_locale_to_language() {
        assert_eq!(locale_to_language(&Locale::All), None);
        assert_eq!(locale_to_language(&Locale::EnUS), Some("english"));
        assert_eq!(locale_to_language(&Locale::EnGB), Some("english_uk"));
        assert_eq!(locale_to_language(&Locale::TrTR), Some("turkish"));
    }

    #[test]
    fn test_extr() {
        assert_eq!(
            extr(
                "React.createElement(UIStartpage.AppSerpWeb, {\"foo\":1}})",
                "React.createElement(UIStartpage.AppSerpWeb, {",
                "}})"
            ),
            Some("\"foo\":1".to_string())
        );
    }

    #[test]
    fn test_parse_published_date_prefix() {
        assert_eq!(
            parse_published_date_prefix("2 Sep 2014 ... Some content here"),
            "Some content here"
        );
        assert_eq!(
            parse_published_date_prefix("5 days ago ... More content"),
            "More content"
        );
        assert_eq!(
            parse_published_date_prefix("No date prefix here"),
            "No date prefix here"
        );
    }
}
