//! Yahoo Search (Web)
//!
//! Port of `SearXNG`'s yahoo.py engine. Supports web search at <https://search.yahoo.com/>
//!
//! Languages are supported by mapping the language to a domain. If domain is not found in
//! `region2domain`, URL `<lang>.search.yahoo.com` is used.

use std::error::Error;

use reqwest::{
    Method, Url,
    header::{HeaderName, HeaderValue},
};
use scraper::{Html, Selector};

use super::{
    build_sb_cookie, extract_text, html_to_text, lang_to_domain, locale_to_region,
    locale_to_yahoo_lang, parse_url, region_to_domain, safesearch_to_vm, time_range_to_btf,
};
use crate::engine::{SearchParams, SearchProvider, SearchResult};

/// Stateful Yahoo search provider implementing `SearchProvider`.
#[derive(Debug)]
pub struct Yahoo {
    results: Vec<SearchResult>,
    domain: String,
}

impl Default for Yahoo {
    fn default() -> Self {
        Self {
            results: Vec::with_capacity(32),
            domain: String::new(),
        }
    }
}

impl Yahoo {
    /// Resolve domain from locale (region first, then language).
    fn resolve_domain(region: Option<&str>, lang: &str) -> String {
        if let Some(reg) = region
            && let Some(domain) = region_to_domain(reg)
        {
            return domain.to_string();
        }
        lang_to_domain(lang).to_string()
    }
}

impl SearchProvider for Yahoo {
    fn name() -> &'static str {
        "yahoo"
    }

    fn build_request(
        &mut self,
        params: &SearchParams,
    ) -> Result<reqwest::Request, Box<dyn Error + Send + Sync>> {
        let lang = locale_to_yahoo_lang(&params.locale);
        let region = locale_to_region(&params.locale);
        let domain = Self::resolve_domain(region, lang);
        self.domain.clone_from(&domain);

        let mut url = Url::parse(&format!("https://{domain}/search"))
            .map_err(|e| std::io::Error::other(e.to_string()))?;

        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("p", &params.query);

            if let Some(btf) = time_range_to_btf(params.time_range) {
                pairs.append_pair("btf", btf);
            }

            // Page 1: iscqry=''
            pairs.append_pair("iscqry", "");
        }

        let vl = format!("lang_{lang}");
        let sbcookie_params: Vec<(&str, &str)> = vec![
            ("v", "1"),
            ("vm", safesearch_to_vm(params.safesearch)),
            ("fl", "1"),
            ("vl", &vl),
            ("pn", "10"),
            ("rw", "new"),
            ("userset", "1"),
        ];
        let sbcookie = build_sb_cookie(&sbcookie_params);

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
        headers.insert(
            HeaderName::from_static("cookie"),
            HeaderValue::try_from(format!("sB={sbcookie}"))
                .map_err(|e| std::io::Error::other(e.to_string()))?,
        );

        Ok(request)
    }

    fn parse_response(&mut self, body: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        let doc = Html::parse_document(body);

        let results_selector = Selector::parse(r"div.algo-sr").unwrap();
        let comp_text_selector = Selector::parse(r"div.compText").unwrap();

        let url_selector = if self.domain == "search.yahoo.com" {
            Selector::parse(r"div.compTitle a").unwrap()
        } else {
            Selector::parse(r"div.compTitle h3 a").unwrap()
        };

        let mut results = Vec::new();

        for result in doc.select(&results_selector) {
            let url_elem = result.select(&url_selector).next();
            let Some(url_elem) = url_elem else {
                continue;
            };

            let Some(url) = url_elem.value().attr("href") else {
                continue;
            };
            let url = parse_url(url);

            let title = if self.domain == "search.yahoo.com" {
                result
                    .select(&Selector::parse(r"div.compTitle a h3 span").unwrap())
                    .next()
                    .map(extract_text)
                    .unwrap_or_default()
            } else {
                url_elem
                    .value()
                    .attr("aria-label")
                    .map_or_else(|| extract_text(url_elem), ToString::to_string)
            };
            let title = html_to_text(&title);

            let content = result
                .select(&comp_text_selector)
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
