//! DuckDuckGo WEB search engine
//!
//! Port of SearXNG's duckduckgo.py engine.
//! Uses the HTML API at https://html.duckduckgo.com/html/

use reqwest::header::{HeaderName, HeaderValue};
use reqwest::Method;
use scraper::{Html, Selector};
use std::collections::HashMap;
use std::error::Error;

use crate::engine::{Locale, SearchParams, TimeRange};

const BASE_URL: &str = "https://html.duckduckgo.com/html/";
const DDG_SEARCH_URL: &str = "https://duckduckgo.com/";

/// DuckDuckGo region code from Locale: All -> "wt-wt"; otherwise lowercased with hyphens.
fn locale_to_ddg_region(locale: &Locale) -> String {
    match locale {
        Locale::All => "wt-wt".to_string(),
        Locale::EnUS => "en-us".to_string(),
        Locale::EnGB => "en-gb".to_string(),
        Locale::TrTR => "tr-tr".to_string(),
        Locale::Other(s) => s.to_lowercase().replace('_', "-"),
    }
}

fn time_range_to_ddg_code(tr: TimeRange) -> &'static str {
    match tr {
        TimeRange::Any => "",
        TimeRange::Day => "d",
        TimeRange::Week => "w",
        TimeRange::Month => "m",
        TimeRange::Year => "y",
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

/// Builds the form data for the DuckDuckGo POST request
fn build_form_data(
    query: &str,
    page: u32,
    region: &str,
    time_range: TimeRange,
    vqd: Option<&str>,
) -> Result<HashMap<String, String>, Box<dyn Error>> {
    let mut data: HashMap<String, String> = HashMap::new();

    data.insert("q".to_string(), query.to_string());
    data.insert("v".to_string(), "l".to_string());
    data.insert("o".to_string(), "json".to_string());
    data.insert("api".to_string(), "d.js".to_string());
    data.insert("kl".to_string(), region.to_string());
    data.insert(
        "df".to_string(),
        time_range_to_ddg_code(time_range).to_string(),
    );

    if page == 1 {
        data.insert("b".to_string(), String::new());
        if let Some(v) = vqd {
            data.insert("vqd".to_string(), v.to_string());
        }
    } else {
        // Page 2 = offset 10, Page 3+ = 10 + (page - 2) * 15
        let offset = 10 + (page.saturating_sub(2)) * 15;
        data.insert("s".to_string(), offset.to_string());
        data.insert("nextParams".to_string(), String::new());
        data.insert("dc".to_string(), (offset + 1).to_string());

        if let Some(v) = vqd {
            data.insert("vqd".to_string(), v.to_string());
        } else {
            return Err("VQD required for pagination but could not be obtained".into());
        }
    }

    Ok(data)
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

/// Phase of the DuckDuckGo state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DdgPhase {
    /// Need to fetch VQD token (GET to duckduckgo.com)
    NeedVqd,
    /// Ready to search (POST to html.duckduckgo.com)
    NeedSearch,
    /// No more requests
    Done,
}

/// Stateful DuckDuckGo search provider implementing SearchProvider.
#[derive(Debug)]
pub struct DuckDuckGo {
    params: Option<SearchParams>,
    phase: DdgPhase,
    vqd: Option<String>,
    results: Vec<crate::engine::SearchResult>,
}

impl Default for DuckDuckGo {
    fn default() -> Self {
        Self {
            params: None,
            phase: DdgPhase::NeedVqd,
            vqd: None,
            results: Vec::with_capacity(32),
        }
    }
}

impl DuckDuckGo {
    pub fn new() -> Self {
        Self::default()
    }

    fn build_vqd_request(
        params: &SearchParams,
    ) -> Result<reqwest::Request, Box<dyn Error + Send + Sync>> {
        let query_string = serde_urlencoded::to_string([("q", params.query.as_str())])
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        let url = format!("{}?{}", DDG_SEARCH_URL, query_string);
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

    fn build_search_request(
        &self,
        params: &SearchParams,
    ) -> Result<reqwest::Request, Box<dyn Error + Send + Sync>> {
        let region = locale_to_ddg_region(&params.locale);
        let form_data = build_form_data(
            &params.query,
            1,
            &region,
            params.time_range,
            self.vqd.as_deref(),
        )
        .map_err(|e| std::io::Error::other(e.to_string()))?;
        let body = serde_urlencoded::to_string(&form_data)
            .map_err(|e| std::io::Error::other(e.to_string()))?;

        let url =
            reqwest::Url::parse(BASE_URL).map_err(|e| std::io::Error::other(e.to_string()))?;
        let mut request = reqwest::Request::new(Method::POST, url);
        let headers = request.headers_mut();
        headers.insert(
            HeaderName::from_static("content-type"),
            HeaderValue::from_static("application/x-www-form-urlencoded"),
        );
        headers.insert(
            HeaderName::from_static("referer"),
            HeaderValue::from_static(BASE_URL),
        );
        headers.insert(
            HeaderName::from_static("sec-fetch-dest"),
            HeaderValue::from_static("document"),
        );
        headers.insert(
            HeaderName::from_static("sec-fetch-mode"),
            HeaderValue::from_static("navigate"),
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
            HeaderName::from_static("accept-language"),
            HeaderValue::from_static("en-US,en;q=0.9"),
        );
        headers.insert(
            HeaderName::from_static("user-agent"),
            HeaderValue::from_static(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
            ),
        );
        let cookie_value = format!("kl={}", region);
        headers.insert(
            HeaderName::from_static("cookie"),
            HeaderValue::try_from(cookie_value)
                .map_err(|e| std::io::Error::other(e.to_string()))?,
        );
        *request.body_mut() = Some(reqwest::Body::from(body));

        Ok(request)
    }
}

impl crate::engine::SearchProvider for DuckDuckGo {
    fn build_request(
        &mut self,
        params: Option<crate::engine::SearchParams>,
    ) -> Result<Option<reqwest::Request>, Box<dyn Error + Send + Sync>> {
        let params = params.or_else(|| self.params.clone());
        let params = match params {
            Some(p) => p,
            None => return Ok(None),
        };

        if params.query.len() >= 500 {
            return Err("Query too long (max 499 characters)".into());
        }

        if self.params.is_none() {
            self.params = Some(params.clone());
        }

        match self.phase {
            DdgPhase::NeedVqd => {
                let req = Self::build_vqd_request(&params)?;
                Ok(Some(req))
            }
            DdgPhase::NeedSearch => {
                let req = self.build_search_request(&params)?;
                self.phase = DdgPhase::Done;
                Ok(Some(req))
            }
            DdgPhase::Done => Ok(None),
        }
    }

    fn parse_response(&mut self, body: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        if self.phase == DdgPhase::NeedVqd {
            self.vqd = extr(body, "vqd=\"", "\"");
            self.phase = DdgPhase::NeedSearch;
            return Ok(());
        }

        match parse_response(body) {
            Ok(response) => {
                for r in response.results {
                    self.results.push(crate::engine::SearchResult {
                        title: r.title,
                        url: r.url,
                        content: r.content,
                    });
                }
                Ok(())
            }
            Err(e) if e.to_string().contains("CAPTCHA") => Ok(()),
            Err(e) => Err(std::io::Error::other(e.to_string()).into()),
        }
    }

    fn results(
        &mut self,
    ) -> Option<Result<Vec<crate::engine::SearchResult>, Box<dyn Error + Send + Sync>>> {
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
    use crate::engine::TimeRange;

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
        assert_eq!(time_range_to_ddg_code(TimeRange::Any), "");
        assert_eq!(time_range_to_ddg_code(TimeRange::Day), "d");
        assert_eq!(time_range_to_ddg_code(TimeRange::Week), "w");
        assert_eq!(time_range_to_ddg_code(TimeRange::Month), "m");
        assert_eq!(time_range_to_ddg_code(TimeRange::Year), "y");
    }

    #[test]
    fn test_locale_to_ddg_region() {
        use crate::engine::Locale;
        assert_eq!(locale_to_ddg_region(&Locale::All), "wt-wt");
        assert_eq!(locale_to_ddg_region(&Locale::EnUS), "en-us");
        assert_eq!(locale_to_ddg_region(&Locale::EnGB), "en-gb");
        assert_eq!(locale_to_ddg_region(&Locale::TrTR), "tr-tr");
        assert_eq!(
            locale_to_ddg_region(&Locale::Other("en_US".to_string())),
            "en-us"
        );
    }
}
