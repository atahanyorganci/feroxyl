//! Bing search engine
//!
//! Port of `SearXNG`'s bing.py engine. Supports web search at <https://www.bing.com/search>

use std::{
    error::Error,
    time::{SystemTime, UNIX_EPOCH},
};

use base64::Engine;
use reqwest::{
    header::{HeaderName, HeaderValue},
    Method, Url,
};
use scraper::{ElementRef, Html, Selector};

use crate::engine::{Locale, SearchParams, SearchProvider, SearchResult, TimeRange};

const BASE_URL: &str = "https://www.bing.com/search";

/// Page offset for Bing pagination: (`page_no` - 1) * 10 + 1
fn page_offset(page_no: u32) -> u32 {
    (page_no.saturating_sub(1)) * 10 + 1
}

/// Bing engine region from Locale (market code, e.g. "en-us", "en-gb").
/// `Locale::Other` defaults to en-us.
fn locale_to_region(locale: &Locale) -> &'static str {
    match locale {
        Locale::All | Locale::EnUS | Locale::Other(_) => "en-us",
        Locale::EnGB => "en-gb",
        Locale::TrTR => "tr-tr",
    }
}

/// Bing engine language from Locale (UI language).
fn locale_to_language(locale: &Locale) -> &'static str {
    locale_to_region(locale)
}

/// Time range to Bing filters ex1 code.
fn time_range_to_bing_filter(tr: TimeRange) -> Option<String> {
    match tr {
        TimeRange::Any => None,
        TimeRange::Day => Some("ez1".to_string()),
        TimeRange::Week => Some("ez2".to_string()),
        TimeRange::Month => Some("ez3".to_string()),
        TimeRange::Year => {
            let unix = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            let unix_day = unix / 86400;
            Some(format!("ez5_{}_{}", unix_day - 365, unix_day))
        }
    }
}

/// Decode Bing redirect URL (<https://www.bing.com/ck/a>?...) to real URL.
fn decode_bing_redirect_url(url: &str) -> Result<String, Box<dyn Error + Send + Sync>> {
    let url_obj = Url::parse(url).map_err(|e| std::io::Error::other(e.to_string()))?;
    let param_u = url_obj
        .query_pairs()
        .find(|(k, _)| k == "u")
        .map(|(_, v)| v.to_string())
        .ok_or_else(|| std::io::Error::other("Missing 'u' parameter in Bing redirect URL"))?;

    // Remove "a1" prefix (Python: encoded_url = param_u[2:])
    let encoded = param_u.strip_prefix("a1").unwrap_or(&param_u);

    // Add padding (Python: encoded_url + '=' * (-len(encoded_url) % 4))
    let pad_len = (4 - encoded.len() % 4) % 4;
    let padded = format!("{}{}", encoded, "=".repeat(pad_len));

    let decoded = base64::engine::general_purpose::URL_SAFE
        .decode(padded.as_bytes())
        .map_err(|e| std::io::Error::other(e.to_string()))?;

    String::from_utf8(decoded).map_err(|e| std::io::Error::other(e.to_string()).into())
}

/// Extract text from an element (all descendant text nodes concatenated).
fn extract_text(element: ElementRef) -> String {
    element.text().collect::<String>().trim().to_string()
}

/// Parse Bing web search HTML response.
fn parse_search_response(
    html: &str,
    expected_start: u32,
) -> Result<Vec<SearchResult>, Box<dyn Error + Send + Sync>> {
    let doc = Html::parse_document(html);

    let results_selector = Selector::parse(r"ol#b_results li.b_algo").unwrap();
    let link_selector = Selector::parse("h2 a").unwrap();
    let content_selector = Selector::parse("p").unwrap();

    let mut results = Vec::new();

    for result in doc.select(&results_selector) {
        let link = result.select(&link_selector).next();
        let Some(link) = link else {
            continue;
        };

        let mut url = link.value().attr("href").unwrap_or("").to_string();
        if url.is_empty() {
            continue;
        }

        let title = extract_text(link);

        // Content: concatenate text from all p elements; algoSlug_icon adds "Web" which we strip
        let mut content_parts = Vec::new();
        for p in result.select(&content_selector) {
            let mut text = extract_text(p);
            if text.ends_with(" Web") {
                text = text.strip_suffix(" Web").unwrap_or(&text).to_string();
            }
            if !text.is_empty() {
                content_parts.push(text);
            }
        }
        let content = if content_parts.is_empty() {
            None
        } else {
            Some(content_parts.join(" ").trim().to_string())
        };
        let content = content.filter(|s| !s.is_empty());

        // Decode redirect URL if needed
        if url.starts_with("https://www.bing.com/ck/a?") {
            match decode_bing_redirect_url(&url) {
                Ok(decoded) => url = decoded,
                Err(e) => {
                    tracing::debug!(url = %url, error = %e, "Failed to decode Bing redirect URL");
                    continue;
                }
            }
        }

        results.push(SearchResult {
            title,
            url,
            content,
        });
    }

    // Rate limit check: verify we got the expected page
    if !results.is_empty() {
        let sb_count_selector = Selector::parse("span.sb_count").unwrap();
        if let Some(sb_span) = doc.select(&sb_count_selector).next() {
            let count_text: String = sb_span.text().collect();
            let (start, result_len) = parse_sb_count(&count_text);

            if expected_start > result_len {
                return Ok(vec![]);
            }
            if expected_start != start {
                return Err(std::io::Error::other(format!(
                    "Bing rate limit: expected results to start at {expected_start}, but got {start}"
                ))
                .into());
            }
        }
    }

    Ok(results)
}

/// Parse `sb_count` span text to extract (start, `result_len`).
/// Mimics Python: split by r'-\d+', first part = start, second part = strip non-digits for `result_len`.
fn parse_sb_count(text: &str) -> (u32, u32) {
    let text = text.trim();
    let (start, rest) = if let Some(hyphen_pos) = text.find('-') {
        // Format: "1-10 of 1,234" or "11-20 of 500" - split by "-\d+" (hyphen + digits)
        let start_str = text[..hyphen_pos].trim();
        let after_hyphen = &text[hyphen_pos + 1..];
        // Skip digits immediately after hyphen (Python: -\d+)
        let digits_len = after_hyphen
            .chars()
            .take_while(char::is_ascii_digit)
            .count();
        let rest = &after_hyphen[digits_len..];
        let start = start_str
            .chars()
            .filter(char::is_ascii_digit)
            .collect::<String>()
            .parse()
            .unwrap_or(1);
        (start, rest)
    } else {
        (1u32, text)
    };

    let result_len: u32 = rest
        .chars()
        .filter(char::is_ascii_digit)
        .collect::<String>()
        .parse()
        .unwrap_or(0);

    (start, result_len)
}

/// Stateful Bing search provider implementing `SearchProvider`.
#[derive(Debug)]
pub struct Bing {
    results: Vec<SearchResult>,
}

impl Default for Bing {
    fn default() -> Self {
        Self {
            results: Vec::with_capacity(32),
        }
    }
}

impl SearchProvider for Bing {
    fn name() -> &'static str {
        "bing"
    }

    fn build_request(
        &mut self,
        params: &SearchParams,
    ) -> Result<reqwest::Request, Box<dyn Error + Send + Sync>> {
        let mut url = Url::parse(BASE_URL).map_err(|e| std::io::Error::other(e.to_string()))?;

        let page = 1u32; // First page only for initial port
        let region = locale_to_region(&params.locale);
        let language = locale_to_language(&params.locale);

        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("q", &params.query);
            pairs.append_pair("pq", &params.query);

            if page > 1 {
                pairs.append_pair("first", &page_offset(page).to_string());
            }
            if page == 2 {
                pairs.append_pair("FORM", "PERE");
            } else if page > 2 {
                pairs.append_pair("FORM", &format!("PERE{}", page - 2));
            }

            if let Some(ref filter) = time_range_to_bing_filter(params.time_range) {
                pairs.append_pair("filters", &format!("ex1:\"{filter}\""));
            }
        }

        let mut request = reqwest::Request::new(Method::GET, url);

        let headers = request.headers_mut();
        headers.insert(
            HeaderName::from_static("accept"),
            HeaderValue::from_static(
                "text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,*/*;q=0.8",
            ),
        );
        headers.insert(
            HeaderName::from_static("accept-language"),
            HeaderValue::from_static("en-US;q=0.5,en;q=0.3"),
        );
        headers.insert(
            HeaderName::from_static("dnt"),
            HeaderValue::from_static("1"),
        );
        headers.insert(
            HeaderName::from_static("upgrade-insecure-requests"),
            HeaderValue::from_static("1"),
        );
        headers.insert(
            HeaderName::from_static("sec-gpc"),
            HeaderValue::from_static("1"),
        );
        headers.insert(
            HeaderName::from_static("cache-control"),
            HeaderValue::from_static("max-age=0"),
        );
        headers.insert(
            HeaderName::from_static("user-agent"),
            HeaderValue::from_static(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
            ),
        );

        // Bing cookies for locale
        let edge_cd = format!("m={region}&u={language}");
        let edge_s = format!("mkt={region}&ui={language}");
        let cookie_value = format!("_EDGE_CD={edge_cd}; _EDGE_S={edge_s}");
        headers.insert(
            HeaderName::from_static("cookie"),
            HeaderValue::try_from(cookie_value)
                .map_err(|e| std::io::Error::other(e.to_string()))?,
        );

        Ok(request)
    }

    fn parse_response(&mut self, body: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        let expected_start = page_offset(1);
        match parse_search_response(body, expected_start) {
            Ok(results) => {
                self.results = results;
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    fn results(&mut self) -> Option<Result<Vec<SearchResult>, Box<dyn Error + Send + Sync>>> {
        if self.results.is_empty() {
            Some(Err("No results found".into()))
        } else {
            Some(Ok(std::mem::take(&mut self.results)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_page_offset() {
        assert_eq!(page_offset(1), 1);
        assert_eq!(page_offset(2), 11);
        assert_eq!(page_offset(3), 21);
    }

    #[test]
    fn test_parse_sb_count() {
        assert_eq!(parse_sb_count("1-10 of 1,234 results"), (1, 1234));
        assert_eq!(parse_sb_count("11-20 of 500"), (11, 500));
        assert_eq!(parse_sb_count("About 42 results"), (1, 42));
    }

    #[test]
    fn test_locale_to_region() {
        assert_eq!(locale_to_region(&Locale::All), "en-us");
        assert_eq!(locale_to_region(&Locale::EnUS), "en-us");
        assert_eq!(locale_to_region(&Locale::EnGB), "en-gb");
        assert_eq!(locale_to_region(&Locale::TrTR), "tr-tr");
        assert_eq!(locale_to_region(&Locale::Other("fr-FR".into())), "en-us");
    }

    #[test]
    fn test_time_range_to_bing_filter() {
        assert_eq!(time_range_to_bing_filter(TimeRange::Any), None);
        assert_eq!(
            time_range_to_bing_filter(TimeRange::Day),
            Some("ez1".to_string())
        );
        assert_eq!(
            time_range_to_bing_filter(TimeRange::Week),
            Some("ez2".to_string())
        );
        assert_eq!(
            time_range_to_bing_filter(TimeRange::Month),
            Some("ez3".to_string())
        );
    }
}
