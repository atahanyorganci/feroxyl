//! Brave search engine
//!
//! Port of `SearXNG`'s brave.py engine. Supports web search (category "search").
//! Uses HTML scraping at <https://search.brave.com>

use reqwest::Method;
use reqwest::Url;
use reqwest::header::{HeaderName, HeaderValue};
use scraper::{ElementRef, Html, Selector};
use std::error::Error;

use crate::engine::{Locale, Safesearch, SearchParams, SearchProvider, SearchResult, TimeRange};

/// Time range to Brave tf param (pd, pw, pm, py).
fn time_range_to_brave(tr: TimeRange) -> Option<&'static str> {
    match tr {
        TimeRange::Any => None,
        TimeRange::Day => Some("pd"),
        TimeRange::Week => Some("pw"),
        TimeRange::Month => Some("pm"),
        TimeRange::Year => Some("py"),
    }
}

/// Safesearch to Brave cookie value.
fn safesearch_to_brave(s: Safesearch) -> &'static str {
    match s {
        Safesearch::Off => "off",
        Safesearch::Moderate => "moderate",
        Safesearch::Strict => "strict",
    }
}

/// Country cookie from locale (e.g. "en-CA" -> "ca", "all" -> "all").
fn locale_to_country(locale: &Locale) -> &'static str {
    match locale {
        Locale::EnUS => "us",
        Locale::EnGB => "gb",
        Locale::TrTR => "tr",
        Locale::All | Locale::Other(_) => "all",
    }
}

/// UI language cookie from locale (e.g. "en-US" -> "en-us").
fn locale_to_ui_lang(locale: &Locale) -> &'static str {
    match locale {
        Locale::EnUS => "en-us",
        Locale::EnGB => "en-gb",
        Locale::TrTR => "tr-tr",
        Locale::All | Locale::Other(_) => "all",
    }
}

/// Extract text from an element (all descendant text nodes concatenated).
fn extract_text(element: ElementRef) -> String {
    element.text().collect::<String>().trim().to_string()
}

/// Parse Brave web search HTML response.
fn parse_search_response(html: &str) -> Vec<SearchResult> {
    let doc = Html::parse_document(html);

    // Only web result snippets (excludes related-queries which also have class "snippet")
    let snippet_selector = Selector::parse(r#"div[data-type="web"]"#).unwrap();
    let link_selector = Selector::parse(r#"a[href^="http"]"#).unwrap();
    let title_selector =
        Selector::parse(r#"div[class*="snippet-title"], div[class*="title"]"#).unwrap();
    let content_selector =
        Selector::parse(r#"div[class~="content"], div[class~="description"]"#).unwrap();

    let mut results = Vec::new();

    for snippet in doc.select(&snippet_selector) {
        // URL: first <a href="..."> - skip if relative (ad)
        let url = snippet
            .select(&link_selector)
            .next()
            .and_then(|a| a.value().attr("href").map(String::from));

        let Some(url) = url else {
            continue;
        };

        // Title: div with snippet-title or title in class; fallback to link text
        let title = snippet
            .select(&title_selector)
            .next()
            .map(extract_text)
            .filter(|s| !s.is_empty())
            .or_else(|| {
                snippet
                    .select(&link_selector)
                    .next()
                    .map(extract_text)
                    .filter(|s| !s.is_empty())
            });

        let Some(title) = title else {
            continue;
        };

        // Content: div with class "content" (word match to avoid site-name-content)
        let content = snippet
            .select(&content_selector)
            .next()
            .map(|el| {
                let mut text = extract_text(el);
                // Strip published date from content if present (span.t-secondary)
                let t_secondary = Selector::parse("span[class*=\"t-secondary\"]").ok();
                if let Some(sel) = t_secondary {
                    if let Some(span) = el.select(&sel).next() {
                        let pub_text = extract_text(span);
                        if !pub_text.is_empty() {
                            text = text
                                .strip_prefix(&pub_text)
                                .unwrap_or(&text)
                                .trim_start_matches(|c| {
                                    c == '-' || c == ' ' || c == '\n' || c == '\t'
                                })
                                .to_string();
                        }
                    }
                }
                text
            })
            .filter(|s| !s.is_empty());

        results.push(SearchResult {
            title,
            url,
            content,
        });
    }

    results
}

/// Stateful Brave search provider implementing `SearchProvider`.
#[derive(Debug)]
pub struct Brave {
    results: Vec<SearchResult>,
}

impl Default for Brave {
    fn default() -> Self {
        Self {
            results: Vec::with_capacity(32),
        }
    }
}

impl SearchProvider for Brave {
    fn name() -> &'static str {
        "brave"
    }

    fn build_request(
        &mut self,
        params: &SearchParams,
    ) -> Result<reqwest::Request, Box<dyn Error + Send + Sync>> {
        let mut url = Url::parse("https://search.brave.com/search").unwrap();
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("q", &params.query);
            pairs.append_pair("source", "web");

            if let Some(tf) = time_range_to_brave(params.time_range) {
                pairs.append_pair("tf", tf);
            }
        }

        let mut request = reqwest::Request::new(Method::GET, url);

        let headers = request.headers_mut();
        headers.insert(
            HeaderName::from_static("accept"),
            HeaderValue::from_static(
                "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8",
            ),
        );
        headers.insert(
            HeaderName::from_static("accept-encoding"),
            HeaderValue::from_static("gzip, deflate, br"),
        );
        headers.insert(
            HeaderName::from_static("accept-language"),
            HeaderValue::from_static("en-US,en;q=0.9"),
        );
        headers.insert(
            HeaderName::from_static("sec-ch-ua"),
            HeaderValue::from_static(
                "\"Not_A Brand\";v=\"8\", \"Chromium\";v=\"120\", \"Google Chrome\";v=\"120\"",
            ),
        );
        headers.insert(
            HeaderName::from_static("sec-ch-ua-mobile"),
            HeaderValue::from_static("?0"),
        );
        headers.insert(
            HeaderName::from_static("sec-ch-ua-platform"),
            HeaderValue::from_static("\"Windows\""),
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
            HeaderValue::from_static("none"),
        );
        headers.insert(
            HeaderName::from_static("sec-fetch-user"),
            HeaderValue::from_static("?1"),
        );
        headers.insert(
            HeaderName::from_static("upgrade-insecure-requests"),
            HeaderValue::from_static("1"),
        );
        headers.insert(
            HeaderName::from_static("user-agent"),
            HeaderValue::from_static(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
            ),
        );

        // Cookies: safesearch, useLocation, summarizer, country, ui_lang
        let country = locale_to_country(&params.locale);
        let ui_lang = locale_to_ui_lang(&params.locale);
        let cookie_value = format!(
            "safesearch={}; useLocation=0; summarizer=0; country={}; ui_lang={}",
            safesearch_to_brave(params.safesearch),
            country,
            ui_lang
        );
        headers.insert(
            HeaderName::from_static("cookie"),
            HeaderValue::try_from(cookie_value)
                .map_err(|e| std::io::Error::other(e.to_string()))?,
        );

        Ok(request)
    }

    fn parse_response(&mut self, body: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.results = parse_search_response(body);
        Ok(())
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
    fn test_time_range_to_brave() {
        assert_eq!(time_range_to_brave(TimeRange::Any), None);
        assert_eq!(time_range_to_brave(TimeRange::Day), Some("pd"));
        assert_eq!(time_range_to_brave(TimeRange::Week), Some("pw"));
        assert_eq!(time_range_to_brave(TimeRange::Month), Some("pm"));
        assert_eq!(time_range_to_brave(TimeRange::Year), Some("py"));
    }

    #[test]
    fn test_safesearch_to_brave() {
        assert_eq!(safesearch_to_brave(Safesearch::Off), "off");
        assert_eq!(safesearch_to_brave(Safesearch::Moderate), "moderate");
        assert_eq!(safesearch_to_brave(Safesearch::Strict), "strict");
    }

    #[test]
    fn test_locale_to_country() {
        assert_eq!(locale_to_country(&Locale::All), "all");
        assert_eq!(locale_to_country(&Locale::EnUS), "us");
        assert_eq!(locale_to_country(&Locale::EnGB), "gb");
        assert_eq!(locale_to_country(&Locale::TrTR), "tr");
        assert_eq!(
            locale_to_country(&Locale::Other("en-CA".to_string())),
            "all"
        );
        assert_eq!(
            locale_to_country(&Locale::Other("unknown".to_string())),
            "all"
        );
    }

    #[test]
    fn test_locale_to_ui_lang() {
        assert_eq!(locale_to_ui_lang(&Locale::All), "all");
        assert_eq!(locale_to_ui_lang(&Locale::EnUS), "en-us");
        assert_eq!(locale_to_ui_lang(&Locale::EnGB), "en-gb");
        assert_eq!(locale_to_ui_lang(&Locale::TrTR), "tr-tr");
        assert_eq!(
            locale_to_ui_lang(&Locale::Other("en-CA".to_string())),
            "all"
        );
    }
}
