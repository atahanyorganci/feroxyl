//! Yahoo News search engine
//!
//! Port of `SearXNG`'s `yahoo_news.py` engine.
//! Yahoo News is "English only" and does not offer localized nor language queries.
//! Search URL: <https://news.search.yahoo.com/search>

use std::{collections::HashSet, error::Error};

use reqwest::{
    Method, Url,
    header::{HeaderName, HeaderValue},
};
use scraper::{Html, Selector};

use super::{extract_text, html_to_text, parse_url};
use crate::engine::{SearchParams, SearchProvider, SearchResult};

const SEARCH_URL: &str = "https://news.search.yahoo.com/search";

/// Stateful Yahoo News search provider implementing `SearchProvider`.
#[derive(Debug)]
pub struct YahooNews {
    results: Vec<SearchResult>,
}

impl Default for YahooNews {
    fn default() -> Self {
        Self {
            results: Vec::with_capacity(32),
        }
    }
}

impl SearchProvider for YahooNews {
    fn name() -> &'static str {
        "yahoo_news"
    }

    fn build_request(
        &mut self,
        params: &SearchParams,
    ) -> Result<reqwest::Request, Box<dyn Error + Send + Sync>> {
        // Page 1: offset = 1; Page 2: offset = 11; etc.
        let offset = 1u32;

        let mut url = Url::parse(SEARCH_URL).map_err(|e| std::io::Error::other(e.to_string()))?;
        url.query_pairs_mut()
            .append_pair("p", &params.query)
            .append_pair("b", &offset.to_string());

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

    fn parse_response(&mut self, body: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        let doc = Html::parse_document(body);

        // XPath: //ol[contains(@class,"searchCenterMiddle")]//li
        let results_selector = Selector::parse(r#"ol[class*="searchCenterMiddle"] li"#)
            .unwrap_or_else(|_| Selector::parse("ol.searchCenterMiddle li").unwrap());

        let mut results = Vec::new();
        let mut seen_urls = HashSet::new();

        for result in doc.select(&results_selector) {
            let link_selector = Selector::parse("h4 a").unwrap();
            let Some(link) = result.select(&link_selector).next() else {
                continue;
            };

            let Some(url) = link.value().attr("href") else {
                continue;
            };
            let url = parse_url(url);

            // Deduplicate: nested li elements can produce the same result multiple times
            if !seen_urls.insert(url.clone()) {
                continue;
            }

            let title = extract_text(link);
            let title = html_to_text(&title);

            let content = result
                .select(&Selector::parse("p").unwrap())
                .next()
                .map(extract_text)
                .unwrap_or_default();
            let content = html_to_text(&content);

            results.push(SearchResult {
                title,
                url,
                content: if content.is_empty() {
                    None
                } else {
                    Some(content)
                },
            });
        }

        self.results = results;
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
