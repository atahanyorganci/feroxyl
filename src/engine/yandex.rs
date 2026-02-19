//! Yandex web search engine
//!
//! Port of `SearXNG`'s yandex.py engine with `search_type=web`.
//! Uses the HTML API at <https://yandex.com/search/site/>

use std::error::Error;

use reqwest::{Method, Url};
use scraper::{ElementRef, Html, Selector};

use crate::engine::{Locale, SearchParams, SearchProvider, SearchResult};

const BASE_URL: &str = "https://yandex.com/search/site/";

/// Yandex-supported language codes (BCP 47 primary subtag).
const YANDEX_SUPPORTED_LANGS: &[&str] = &[
    "ru", // Russian
    "en", // English
    "be", // Belarusian
    "fr", // French
    "de", // German
    "id", // Indonesian
    "kk", // Kazakh
    "tt", // Tatar
    "tr", // Turkish
    "uk", // Ukrainian
];

/// Cookie value used by Yandex to avoid some rate limits.
const YANDEX_COOKIE: &str = "yp=1716337604.sp.family%3A0#1685406411.szm.1:1920x1080:1920x999";

/// Extract language code from locale (e.g. "en-US" -> "en").
fn locale_to_lang(locale: &Locale) -> Option<&'static str> {
    let lang = locale.as_str().split('-').next()?;
    YANDEX_SUPPORTED_LANGS.iter().find(|&&l| l == lang).copied()
}

/// Extract text from an element (all descendant text nodes concatenated).
fn extract_text(element: ElementRef) -> String {
    element.text().collect::<String>().trim().to_string()
}

/// Parse Yandex web search HTML response.
fn parse_search_response(html: &str) -> Vec<SearchResult> {
    let doc = Html::parse_document(html);

    let results_selector = Selector::parse(r#"li[class*="serp-item"]"#).unwrap();
    let url_selector = Selector::parse(r"a.b-serp-item__title-link").unwrap();
    let title_selector =
        Selector::parse(r"h3.b-serp-item__title a.b-serp-item__title-link span").unwrap();
    let content_selector =
        Selector::parse(r"div.b-serp-item__content div.b-serp-item__text").unwrap();

    let mut results = Vec::new();

    for result in doc.select(&results_selector) {
        let link = result.select(&url_selector).next();
        let Some(link) = link else {
            continue;
        };

        let url = link.value().attr("href").unwrap_or("").to_string();
        if url.is_empty() {
            continue;
        }

        let title = result
            .select(&title_selector)
            .next()
            .map(extract_text)
            .unwrap_or_default();

        let content = result
            .select(&content_selector)
            .next()
            .map(extract_text)
            .filter(|s| !s.is_empty());

        results.push(SearchResult {
            title,
            url,
            content,
        });
    }

    results
}

/// Stateful Yandex web search provider implementing `SearchProvider`.
#[derive(Debug)]
pub struct Yandex {
    results: Vec<SearchResult>,
}

impl Default for Yandex {
    fn default() -> Self {
        Self {
            results: Vec::with_capacity(32),
        }
    }
}

impl SearchProvider for Yandex {
    fn name() -> &'static str {
        "yandex"
    }

    fn build_request(
        &mut self,
        params: &SearchParams,
    ) -> Result<reqwest::Request, Box<dyn Error + Send + Sync>> {
        let mut url = Url::parse(BASE_URL).map_err(|e| std::io::Error::other(e.to_string()))?;

        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("tmpl_version", "releases");
            pairs.append_pair("text", &params.query);
            pairs.append_pair("web", "1");
            pairs.append_pair("frame", "1");
            pairs.append_pair("searchid", "3131712");

            if let Some(lang) = locale_to_lang(&params.locale) {
                pairs.append_pair("lang", lang);
            }
        }

        let mut request = reqwest::Request::new(Method::GET, url);

        let headers = request.headers_mut();
        headers.insert(
            reqwest::header::HeaderName::from_static("cookie"),
            reqwest::header::HeaderValue::from_static(YANDEX_COOKIE),
        );
        headers.insert(
            reqwest::header::HeaderName::from_static("accept"),
            reqwest::header::HeaderValue::from_static(
                "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            ),
        );
        headers.insert(
            reqwest::header::HeaderName::from_static("accept-language"),
            reqwest::header::HeaderValue::from_static("en-US,en;q=0.9"),
        );
        headers.insert(
            reqwest::header::HeaderName::from_static("user-agent"),
            reqwest::header::HeaderValue::from_static(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
            ),
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
    fn test_locale_to_lang() {
        assert_eq!(locale_to_lang(&Locale::All), None);
        assert_eq!(locale_to_lang(&Locale::EnUS), Some("en"));
        assert_eq!(locale_to_lang(&Locale::TrTR), Some("tr"));
        assert_eq!(locale_to_lang(&Locale::Other("ru-RU".into())), Some("ru"));
        assert_eq!(locale_to_lang(&Locale::Other("de-DE".into())), Some("de"));
        assert_eq!(locale_to_lang(&Locale::Other("zh-CN".into())), None);
    }
}
