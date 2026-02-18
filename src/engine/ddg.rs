//! DuckDuckGo WEB search engine
//!
//! Port of SearXNG's duckduckgo.py engine.
//! Uses the HTML API at https://html.duckduckgo.com/html/

use scraper::{Html, Selector};
use std::collections::HashMap;
use std::error::Error;

const BASE_URL: &str = "https://html.duckduckgo.com/html/";
const DDG_SEARCH_URL: &str = "https://duckduckgo.com/";

/// Time range filter for search results
#[derive(Debug, Clone, Copy, Default)]
#[allow(dead_code)]
pub enum TimeRange {
    #[default]
    Any,
    Day,
    Week,
    Month,
    Year,
}

impl TimeRange {
    fn to_ddg_code(self) -> &'static str {
        match self {
            TimeRange::Any => "",
            TimeRange::Day => "d",
            TimeRange::Week => "w",
            TimeRange::Month => "m",
            TimeRange::Year => "y",
        }
    }
}

/// Parameters for a DuckDuckGo search request
#[derive(Debug, Clone)]
pub struct DuckDuckGoParams {
    /// Search query
    pub query: String,
    /// Page number (1-based)
    pub page: u32,
    /// Region/locale code (e.g. "wt-wt" for all, "en-us" for US English)
    pub region: String,
    /// Optional time range filter
    pub time_range: TimeRange,
}

impl Default for DuckDuckGoParams {
    fn default() -> Self {
        Self {
            query: String::new(),
            page: 1,
            region: "wt-wt".to_string(),
            time_range: TimeRange::Any,
        }
    }
}

/// A single search result from DuckDuckGo
#[derive(Debug, Clone)]
pub struct DuckDuckGoResult {
    pub title: String,
    pub url: String,
    pub content: Option<String>,
}

/// Optional "instant answer" / zero-click result
#[derive(Debug, Clone)]
pub struct ZeroClickAnswer {
    pub answer: String,
    pub url: Option<String>,
}

/// Complete search response
#[derive(Debug, Clone)]
pub struct DuckDuckGoResponse {
    pub results: Vec<DuckDuckGoResult>,
    pub zero_click: Option<ZeroClickAnswer>,
}

/// Extracts text between `begin` and `end` in `txt`
fn extr(txt: &str, begin: &str, end: &str) -> Option<String> {
    let start_idx = txt.find(begin)?;
    let after_begin = start_idx + begin.len();
    let end_idx = txt[after_begin..].find(end)?;
    Some(txt[after_begin..after_begin + end_idx].to_string())
}

/// Fetches the VQD (validation query digest) required for DuckDuckGo's bot protection.
/// The VQD is needed for pagination (page 2+). First page doesn't require it.
async fn get_vqd(
    client: &reqwest::Client,
    query: &str,
    _region: &str,
) -> Result<Option<String>, Box<dyn Error>> {
    let url = format!("{}?q={}", DDG_SEARCH_URL, urlencoding::encode(query));

    let response = client
        .get(&url)
        .header("Accept-Language", "en-US,en;q=0.9")
        .send()
        .await?;

    if !response.status().is_success() {
        return Ok(None);
    }

    let text = response.text().await?;
    let vqd = extr(&text, "vqd=\"", "\"");

    Ok(vqd)
}

/// Builds the form data for the DuckDuckGo POST request
fn build_form_data(
    params: &DuckDuckGoParams,
    vqd: Option<&str>,
) -> Result<HashMap<String, String>, Box<dyn Error>> {
    let mut data: HashMap<String, String> = HashMap::new();

    data.insert("q".to_string(), params.query.clone());
    data.insert("v".to_string(), "l".to_string());
    data.insert("o".to_string(), "json".to_string());
    data.insert("api".to_string(), "d.js".to_string());
    data.insert("kl".to_string(), params.region.clone());
    data.insert(
        "df".to_string(),
        params.time_range.to_ddg_code().to_string(),
    );

    if params.page == 1 {
        data.insert("b".to_string(), String::new());
    } else {
        // Page 2 = offset 10, Page 3+ = 10 + (page - 2) * 15
        let offset = 10 + (params.page.saturating_sub(2)) * 15;
        data.insert("s".to_string(), offset.to_string());
        data.insert("nextParams".to_string(), String::new());
        data.insert("dc".to_string(), (offset + 1).to_string());

        if let Some(v) = vqd {
            data.insert("vqd".to_string(), v.to_string());
        } else {
            // Cannot request page 2+ without VQD - DDG will block the IP
            return Err("VQD required for pagination but could not be obtained".into());
        }
    }

    Ok(data)
}

/// Sends a search request to DuckDuckGo
pub async fn search(
    client: &reqwest::Client,
    params: DuckDuckGoParams,
) -> Result<DuckDuckGoResponse, Box<dyn Error>> {
    // DDG does not accept queries with more than 499 chars
    if params.query.len() >= 500 {
        return Err("Query too long (max 499 characters)".into());
    }

    // For page 2+, we need VQD
    let vqd = if params.page >= 2 {
        get_vqd(client, &params.query, &params.region).await?
    } else {
        None
    };

    // Some locales (e.g. China) don't support "next page"
    if params.page >= 2 && params.region.starts_with("zh") {
        return Ok(DuckDuckGoResponse {
            results: vec![],
            zero_click: None,
        });
    }

    let form_data = build_form_data(&params, vqd.as_deref())?;

    let response = client
        .post(BASE_URL)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("Referer", BASE_URL)
        .header("Sec-Fetch-Dest", "document")
        .header("Sec-Fetch-Mode", "navigate")
        .header("Sec-Fetch-Site", "same-origin")
        .header("Sec-Fetch-User", "?1")
        .header("Accept-Language", "en-US,en;q=0.9")
        .form(&form_data)
        .send()
        .await?;

    // 303 redirect might indicate an issue
    if response.status().as_u16() == 303 {
        return Ok(DuckDuckGoResponse {
            results: vec![],
            zero_click: None,
        });
    }

    let html = response.text().await?;
    parse_response(&html)
}

/// Checks if the response contains a CAPTCHA challenge
fn is_captcha(doc: &Html) -> bool {
    let selector = Selector::parse("form#challenge-form").unwrap();
    doc.select(&selector).next().is_some()
}

/// Parses the HTML response from DuckDuckGo
pub fn parse_response(html: &str) -> Result<DuckDuckGoResponse, Box<dyn Error>> {
    let doc = Html::parse_document(html);

    if is_captcha(&doc) {
        return Err("DuckDuckGo CAPTCHA detected".into());
    }

    let mut results = Vec::new();

    // Select web results: div#links div.web-result (excluding ads: result--ad)
    let links_selector = Selector::parse("#links").unwrap();
    let web_result_selector = Selector::parse("div.web-result").unwrap();
    let title_selector = Selector::parse("h2 a").unwrap();
    let snippet_selector = Selector::parse("a.result__snippet").unwrap();

    if let Some(links_div) = doc.select(&links_selector).next() {
        for div_result in links_div.select(&web_result_selector) {
            // Skip ad results
            let classes = div_result.value().attr("class").unwrap_or("");
            if classes.contains("result--ad") {
                continue;
            }

            let title_elem = div_result.select(&title_selector).next();
            let Some(title_elem) = title_elem else {
                // "No results" item has no title link
                continue;
            };

            let title = title_elem.text().collect::<String>().trim().to_string();
            let url = title_elem.value().attr("href").unwrap_or("").to_string();

            let content = div_result
                .select(&snippet_selector)
                .next()
                .map(|el| el.text().collect::<String>().trim().to_string())
                .filter(|s| !s.is_empty());

            results.push(DuckDuckGoResult {
                title,
                url,
                content,
            });
        }
    }

    // Parse zero-click / instant answer
    let mut zero_click = None;
    let zero_click_selector = Selector::parse("#zero_click_abstract").unwrap();
    if let Some(zc_div) = doc.select(&zero_click_selector).next() {
        let answer = zc_div.text().collect::<String>().trim().to_string();

        // Filter out bot detection messages
        if !answer.is_empty()
            && !answer.contains("Your IP address is")
            && !answer.contains("Your user agent:")
            && !answer.contains("URL Decoded:")
        {
            let url = zc_div
                .select(&Selector::parse("a").unwrap())
                .next()
                .and_then(|a| a.value().attr("href").map(|s| s.to_string()));

            zero_click = Some(ZeroClickAnswer { answer, url });
        }
    }

    Ok(DuckDuckGoResponse {
        results,
        zero_click,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extr() {
        assert_eq!(
            extr("vqd=\"abc123\"", "vqd=\"", "\""),
            Some("abc123".to_string())
        );
        assert_eq!(extr("abcde", "a", "e"), Some("bcd".to_string()));
        assert_eq!(extr("foo", "x", "y"), None);
    }

    #[test]
    fn test_time_range_codes() {
        assert_eq!(TimeRange::Any.to_ddg_code(), "");
        assert_eq!(TimeRange::Day.to_ddg_code(), "d");
        assert_eq!(TimeRange::Week.to_ddg_code(), "w");
        assert_eq!(TimeRange::Month.to_ddg_code(), "m");
        assert_eq!(TimeRange::Year.to_ddg_code(), "y");
    }
}
