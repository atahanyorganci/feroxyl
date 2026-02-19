//! Yahoo search engines (web, news).
//!
//! Shared utilities and re-exports for Yahoo implementations.
//! Port of `SearXNG`'s yahoo.py and `yahoo_news.py` engines.

mod news;
mod web;

pub use news::YahooNews;
use reqwest::Url;
use scraper::{ElementRef, Html};
pub use web::Yahoo;

use crate::engine::{Locale, Safesearch, TimeRange};

/// Yahoo tracking query param to strip from result URLs.
const YAHOO_TRACKING_PARAM: &str = "fr";

/// Map region code to Yahoo domain.
pub(crate) fn region_to_domain(region: &str) -> Option<&'static str> {
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
pub(crate) fn lang_to_domain(lang: &str) -> &'static str {
    match lang {
        "zh_chs" => "hk.search.yahoo.com",
        "zh_cht" => "tw.search.yahoo.com",
        _ => "search.yahoo.com",
    }
}

/// Map Locale to Yahoo language code.
pub(crate) fn locale_to_yahoo_lang(locale: &Locale) -> &'static str {
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
pub(crate) fn locale_to_region(locale: &Locale) -> Option<&str> {
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
pub(crate) fn time_range_to_btf(tr: TimeRange) -> Option<&'static str> {
    match tr {
        TimeRange::Any | TimeRange::Year => None,
        TimeRange::Day => Some("d"),
        TimeRange::Week => Some("w"),
        TimeRange::Month => Some("m"),
    }
}

/// Safesearch to Yahoo vm cookie value (p=off, i=moderate, r=strict).
pub(crate) fn safesearch_to_vm(s: Safesearch) -> &'static str {
    match s {
        Safesearch::Off => "p",
        Safesearch::Moderate => "i",
        Safesearch::Strict => "r",
    }
}

/// Build sB cookie parameter from provided parameters.
pub(crate) fn build_sb_cookie(params: &[(&str, &str)]) -> String {
    params
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("&")
}

/// Remove Yahoo-specific tracking URL. Extract real URL from /RU= to /RS or /RK.
/// Strips `fr=sycsrp_catchall` and similar tracking params from the result.
/// Shared by Yahoo web and news engines.
pub(crate) fn parse_url(url_string: &str) -> String {
    let ru_pos = url_string.find("/RU=");
    let start = match ru_pos {
        Some(p) => url_string[p + 4..].find("http").map(|i| p + 4 + i),
        None => url_string.find("http"),
    };

    let start = match start {
        Some(0) | None => return strip_yahoo_tracking(url_string),
        Some(s) => s,
    };

    let endpositions: Vec<usize> = ["/RS", "/RK"]
        .iter()
        .filter_map(|ending| url_string.rfind(ending))
        .collect();

    if endpositions.is_empty() {
        return strip_yahoo_tracking(url_string);
    }

    let end = *endpositions.iter().min().unwrap();

    let extracted = &url_string[start..end];
    let decoded = urlencoding::decode(extracted)
        .map_or_else(|_| extracted.to_string(), std::borrow::Cow::into_owned);
    strip_yahoo_tracking(&decoded)
}

/// Remove Yahoo tracking query params (e.g. `fr=sycsrp_catchall`) from URL.
fn strip_yahoo_tracking(url_str: &str) -> String {
    let Ok(mut url) = Url::parse(url_str) else {
        return url_str.to_string();
    };
    let pairs: Vec<(String, String)> = url
        .query_pairs()
        .filter(|(k, _)| k != YAHOO_TRACKING_PARAM)
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    if pairs.is_empty() {
        url.set_query(None);
    } else {
        url.query_pairs_mut().clear();
        for (k, v) in pairs {
            url.query_pairs_mut().append_pair(&k, &v);
        }
    }
    url.to_string()
}

/// Strip HTML tags from a string to get plain text.
pub(crate) fn html_to_text(html: &str) -> String {
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
pub(crate) fn extract_text(element: ElementRef) -> String {
    element.text().collect::<String>().trim().to_string()
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

    #[test]
    fn test_parse_url_strips_fr_param() {
        let url = "https://example.com/article?fr=sycsrp_catchall";
        assert_eq!(parse_url(url), "https://example.com/article");
    }

    #[test]
    fn test_parse_url_strips_fr_preserves_other_params() {
        let url = "https://example.com/article?foo=bar&fr=sycsrp_catchall&baz=qux";
        assert_eq!(
            parse_url(url),
            "https://example.com/article?foo=bar&baz=qux"
        );
    }
}
