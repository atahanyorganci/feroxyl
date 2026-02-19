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
use scraper::{ElementRef, Html, Selector};

use crate::engine::{Locale, Safesearch, SearchParams, SearchProvider, SearchResult, TimeRange};

/// Map region code to Yahoo domain.
fn region_to_domain(region: &str) -> Option<&'static str> {
    match region {
        "CO" => Some("co.search.yahoo.com"),
        "TH" => Some("th.search.yahoo.com"),
        "VE" => Some("ve.search.yahoo.com"),
        "CL" => Some("cl.search.yahoo.com"),
        "HK" => Some("hk.search.yahoo.com"),
        "PE" => Some("pe.search.yahoo.com"),
        "CA" => Some("ca.search.yahoo.com"),
        "DE" => Some("de.search.yahoo.com"),
        "FR" => Some("fr.search.yahoo.com"),
        "TW" => Some("tw.search.yahoo.com"),
        "GB" | "UK" => Some("uk.search.yahoo.com"),
        "BR" => Some("br.search.yahoo.com"),
        "IN" => Some("in.search.yahoo.com"),
        "ES" => Some("espanol.search.yahoo.com"),
        "PH" => Some("ph.search.yahoo.com"),
        "AR" => Some("ar.search.yahoo.com"),
        "MX" => Some("mx.search.yahoo.com"),
        "SG" => Some("sg.search.yahoo.com"),
        _ => None,
    }
}

/// Map language code to Yahoo domain (fallback when region has no mapping).
fn lang_to_domain(lang: &str) -> &'static str {
    match lang {
        "zh_chs" => "hk.search.yahoo.com",
        "zh_cht" => "tw.search.yahoo.com",
        _ => "search.yahoo.com",
    }
}

/// Map Locale to Yahoo language code.
fn locale_to_yahoo_lang(locale: &Locale) -> &'static str {
    match locale {
        Locale::All => "any",
        Locale::EnUS | Locale::EnGB => "en",
        Locale::TrTR => "tr",
        Locale::Other(s) => {
            let lang = s.split('-').next().unwrap_or("").to_lowercase();
            match lang.as_str() {
                "ar" => "ar",
                "bg" => "bg",
                "cs" => "cs",
                "da" => "da",
                "de" => "de",
                "el" => "el",
                "en" => "en",
                "es" => "es",
                "et" => "et",
                "fi" => "fi",
                "fr" => "fr",
                "he" => "he",
                "hr" => "hr",
                "hu" => "hu",
                "it" => "it",
                "ja" => "ja",
                "ko" => "ko",
                "lt" => "lt",
                "lv" => "lv",
                "nl" => "nl",
                "no" => "no",
                "pl" => "pl",
                "pt" => "pt",
                "ro" => "ro",
                "ru" => "ru",
                "sk" => "sk",
                "sl" => "sl",
                "sv" => "sv",
                "th" => "th",
                "tr" => "tr",
                "zh" => "zh_chs",
                _ => "any",
            }
        }
    }
}

/// Extract region from Locale (e.g. "DE" from "de-DE").
fn locale_to_region(locale: &Locale) -> Option<&str> {
    match locale {
        Locale::All => None,
        Locale::EnUS => Some("US"),
        Locale::EnGB => Some("GB"),
        Locale::TrTR => Some("TR"),
        Locale::Other(s) => {
            let parts: Vec<&str> = s.split('-').collect();
            if parts.len() >= 2 {
                Some(parts[1])
            } else {
                None
            }
        }
    }
}

/// Time range to Yahoo btf param (d, w, m). Note: Yahoo does not support year filter.
fn time_range_to_btf(tr: TimeRange) -> Option<&'static str> {
    match tr {
        TimeRange::Any | TimeRange::Year => None,
        TimeRange::Day => Some("d"),
        TimeRange::Week => Some("w"),
        TimeRange::Month => Some("m"),
    }
}

/// Safesearch to Yahoo vm cookie value (p=off, i=moderate, r=strict).
fn safesearch_to_vm(s: Safesearch) -> &'static str {
    match s {
        Safesearch::Off => "p",
        Safesearch::Moderate => "i",
        Safesearch::Strict => "r",
    }
}

/// Build sB cookie parameter from provided parameters.
fn build_sb_cookie(params: &[(&str, &str)]) -> String {
    params
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("&")
}

/// Remove Yahoo-specific tracking URL. Extract real URL from /RU= to /RS or /RK.
fn parse_url(url_string: &str) -> String {
    let ru_pos = url_string.find("/RU=");
    let start = match ru_pos {
        Some(p) => url_string[p + 4..].find("http").map(|i| p + 4 + i),
        None => url_string.find("http"),
    };

    let start = match start {
        Some(0) | None => return url_string.to_string(),
        Some(s) => s,
    };

    let endpositions: Vec<usize> = ["/RS", "/RK"]
        .iter()
        .filter_map(|ending| url_string.rfind(ending))
        .collect();

    if endpositions.is_empty() {
        return url_string.to_string();
    }

    let end = *endpositions.iter().min().unwrap();

    let extracted = &url_string[start..end];
    urlencoding::decode(extracted)
        .map_or_else(|_| extracted.to_string(), std::borrow::Cow::into_owned)
}

/// Strip HTML tags from a string to get plain text.
fn html_to_text(html: &str) -> String {
    let fragment = Html::parse_fragment(html);
    fragment
        .root_element()
        .text()
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Extract text from an element (all descendant text nodes concatenated).
fn extract_text(element: ElementRef) -> String {
    element.text().collect::<String>().trim().to_string()
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_region_to_domain() {
        assert_eq!(region_to_domain("DE"), Some("de.search.yahoo.com"));
        assert_eq!(region_to_domain("GB"), Some("uk.search.yahoo.com"));
        assert_eq!(region_to_domain("XX"), None);
    }

    #[test]
    fn test_locale_to_yahoo_lang() {
        assert_eq!(locale_to_yahoo_lang(&Locale::All), "any");
        assert_eq!(locale_to_yahoo_lang(&Locale::EnUS), "en");
        assert_eq!(locale_to_yahoo_lang(&Locale::Other("fr-FR".into())), "fr");
    }

    #[test]
    fn test_time_range_to_btf() {
        assert_eq!(time_range_to_btf(TimeRange::Any), None);
        assert_eq!(time_range_to_btf(TimeRange::Day), Some("d"));
        assert_eq!(time_range_to_btf(TimeRange::Week), Some("w"));
        assert_eq!(time_range_to_btf(TimeRange::Month), Some("m"));
    }

    #[test]
    fn test_safesearch_to_vm() {
        assert_eq!(safesearch_to_vm(Safesearch::Off), "p");
        assert_eq!(safesearch_to_vm(Safesearch::Moderate), "i");
        assert_eq!(safesearch_to_vm(Safesearch::Strict), "r");
    }

    #[test]
    fn test_build_sb_cookie() {
        let params = vec![("v", "1"), ("vm", "p"), ("fl", "1"), ("vl", "lang_fr")];
        assert_eq!(build_sb_cookie(&params), "v=1&vm=p&fl=1&vl=lang_fr");
    }

    #[test]
    fn test_parse_url() {
        let url = "https://example.com/redirect/RU=/RU=https%3A%2F%2Freal-site.com%2Fpath/RS";
        assert!(parse_url(url).contains("real-site.com"));
    }
}
