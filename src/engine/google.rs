//! Google search engine using HTML scraping.

use std::error::Error;

use reqwest::{
    Method, Url,
    header::{HeaderName, HeaderValue},
};
use scraper::{ElementRef, Html, Selector};

use crate::engine::{Locale, Safesearch, SearchParams, SearchProvider, SearchResult, TimeRange};

/// Time range to Google tbs (qdr:) code
fn time_range_to_google_tbs(tr: TimeRange) -> Option<&'static str> {
    match tr {
        TimeRange::Any => None,
        TimeRange::Day => Some("d"),
        TimeRange::Week => Some("w"),
        TimeRange::Month => Some("m"),
        TimeRange::Year => Some("y"),
    }
}

/// Safesearch to Google safe param
fn safesearch_to_google(s: Safesearch) -> &'static str {
    match s {
        Safesearch::Off => "off",
        Safesearch::Moderate => "medium",
        Safesearch::Strict => "high",
    }
}

/// Google hl (interface language) param.
fn locale_to_google_hl(locale: &Locale) -> &str {
    match locale {
        Locale::All | Locale::EnUS => "en-US",
        Locale::EnGB => "en-GB",
        Locale::TrTR => "tr",
        Locale::Other(s) => s.as_str(),
    }
}

/// Google lr (language restriction) param, e.g. "`lang_en`".
fn locale_to_google_lr(locale: &Locale) -> Option<&'static str> {
    match locale {
        Locale::EnUS | Locale::EnGB => Some("lang_en"),
        Locale::TrTR => Some("lang_tr"),
        Locale::All | Locale::Other(_) => None,
    }
}

/// Google cr (country restriction) param, e.g. "countryUS".
fn locale_to_google_cr(locale: &Locale) -> Option<&'static str> {
    match locale {
        Locale::EnUS => Some("countryUS"),
        Locale::EnGB => Some("countryGB"),
        Locale::TrTR => Some("countryTR"),
        Locale::All | Locale::Other(_) => None,
    }
}

fn build_google_search_url(params: &SearchParams) -> Result<Url, Box<dyn Error>> {
    let start = 0u32;
    let mut url = Url::parse("https://www.google.com/search")?;
    {
        let mut pairs = url.query_pairs_mut();
        pairs
            .append_pair("q", &params.query)
            .append_pair("hl", locale_to_google_hl(&params.locale));
        if let Some(lr) = locale_to_google_lr(&params.locale) {
            pairs.append_pair("lr", lr);
        }
        if let Some(cr) = locale_to_google_cr(&params.locale) {
            pairs.append_pair("cr", cr);
        }
        pairs
            .append_pair("ie", "utf8")
            .append_pair("oe", "utf8")
            .append_pair("filter", "0")
            .append_pair("start", &start.to_string());
        if let Some(tbs) = time_range_to_google_tbs(params.time_range) {
            pairs.append_pair("tbs", &format!("qdr:{tbs}"));
        }
        if params.safesearch != Safesearch::Off {
            pairs.append_pair("safe", safesearch_to_google(params.safesearch));
        }
    }
    Ok(url)
}

fn extract_title(element: ElementRef) -> Result<String, Box<dyn Error>> {
    let selector = Selector::parse("div[role*='link']").unwrap();
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

/// Extract the destination URL from a Google result element.
///
/// Google wraps result links in redirects like `/url?q=<real_url>&sa=U&...`.
/// We grab the first `<a>` href, strip the 7-char `/url?q=` prefix, and
/// split on `&sa=U` to isolate the actual URL.
fn extract_url(root: ElementRef) -> Result<String, Box<dyn Error>> {
    let link_selector = Selector::parse("a[href]").unwrap();
    if let Some(link) = root.select(&link_selector).next() {
        let href = link.value().attr("href").unwrap();
        if let Some(raw) = href.strip_prefix("/url?q=") {
            let url = raw.split("&sa=U").next().unwrap_or(raw);
            Ok(urlencoding::decode(url)?.into_owned())
        } else {
            Ok(href.to_string())
        }
    } else {
        Err("No link found".into())
    }
}

fn parse_google_result(element: ElementRef) -> Result<SearchResult, Box<dyn Error>> {
    let title = extract_title(element)?;
    let url = extract_url(element)?;
    let content = extract_content(element);
    if content.is_none() {
        return Err("No content found".into());
    }
    Ok(SearchResult {
        title,
        url,
        content,
    })
}

/// Stateful Google search provider implementing `SearchProvider`.
#[derive(Debug, Default)]
pub struct Google {
    results: Vec<SearchResult>,
}

impl SearchProvider for Google {
    fn name() -> &'static str {
        "google"
    }

    fn build_request(
        &mut self,
        params: &SearchParams,
    ) -> Result<reqwest::Request, Box<dyn Error + Send + Sync>> {
        let url =
            build_google_search_url(params).map_err(|e| std::io::Error::other(e.to_string()))?;
        let mut request = reqwest::Request::new(Method::GET, url);
        let headers = request.headers_mut();
        headers.insert(
            HeaderName::from_static("accept"),
            HeaderValue::from_static("*/*"),
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
        Ok(request)
    }

    fn parse_response(&mut self, body: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        let document = Html::parse_document(body);
        let selector = Selector::parse("div.MjjYud").unwrap();
        let mut skipped = 0u32;
        for result in document.select(&selector) {
            match parse_google_result(result) {
                Ok(r) => self.results.push(r),
                Err(e) => {
                    skipped += 1;
                    tracing::trace!(error = %e, "Skipped unparseable Google result");
                }
            }
        }
        if skipped > 0 {
            tracing::debug!(
                skipped,
                total_candidates = self.results.len() + skipped as usize,
                "Some Google result elements could not be parsed"
            );
        }
        if self.results.is_empty() {
            tracing::warn!("Google returned no parseable results");
            Err("No results found".into())
        } else {
            Ok(())
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
