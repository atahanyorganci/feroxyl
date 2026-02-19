//! Startpage Images search engine
//!
//! Port of `SearXNG`'s startpage.py engine with `startpage_categ: images`.
//! Reuses the same sc-code flow as web search: fetches homepage for sc-code,
//! then POSTs to <https://www.startpage.com/sp/search> with `cat=images`.
//! Parses React `AppSerpImages` JSON embedded in HTML.

use std::{collections::HashMap, error::Error};

use reqwest::{
    header::{HeaderName, HeaderValue},
    Method, Url,
};
use scraper::{Html, Selector};
use serde::Deserialize;

use crate::engine::{
    ImageResult, ImageSearchProvider, Locale, Safesearch, SearchParams, TimeRange,
};

const BASE_URL: &str = "https://www.startpage.com";
const BASE_URL_SLASH: &str = "https://www.startpage.com/";
const SEARCH_URL: &str = "https://www.startpage.com/sp/search";

/// Time range to Startpage `with_date` param (d, w, m, y).
fn time_range_to_startpage(tr: TimeRange) -> &'static str {
    match tr {
        TimeRange::Any => "",
        TimeRange::Day => "d",
        TimeRange::Week => "w",
        TimeRange::Month => "m",
        TimeRange::Year => "y",
    }
}

/// Safesearch to Startpage `disable_family_filter` cookie (0=strict, 1=off).
fn safesearch_to_disable_family_filter(s: Safesearch) -> &'static str {
    match s {
        Safesearch::Off => "1",
        Safesearch::Moderate | Safesearch::Strict => "0",
    }
}

/// Map Locale to Startpage language code.
fn locale_to_language(locale: &Locale) -> Option<&'static str> {
    match locale {
        Locale::All => None,
        Locale::EnUS | Locale::Other(_) => Some("english"),
        Locale::EnGB => Some("english_uk"),
        Locale::TrTR => Some("turkish"),
    }
}

/// Map Locale to Startpage region code.
fn locale_to_region(locale: &Locale) -> Option<&'static str> {
    match locale {
        Locale::EnUS => Some("en-US"),
        Locale::EnGB => Some("en_GB"),
        Locale::TrTR => Some("tr_TR"),
        Locale::All | Locale::Other(_) => None,
    }
}

/// Extract string between `begin` and the last occurrence of `end` in `txt`.
fn extr(txt: &str, begin: &str, end: &str) -> Option<String> {
    let start_idx = txt.find(begin)?;
    let after_begin = start_idx + begin.len();
    let rest = &txt[after_begin..];
    let end_idx = rest.rfind(end)?;
    Some(rest[..end_idx].to_string())
}

/// Strip HTML tags from a string to get plain text.
fn html_to_text(html: &str) -> String {
    let fragment = Html::parse_fragment(html);
    fragment
        .root_element()
        .text()
        .collect::<String>()
        .trim()
        .to_string()
}

/// Humanize bytes to string (e.g. 1024 -> "1 KB").
#[allow(clippy::cast_precision_loss)]
fn humanize_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

/// Build preferences cookie value (N1N-joined keyEEEvalue pairs).
fn build_preferences_cookie(
    safesearch: Safesearch,
    language: Option<&str>,
    region: Option<&str>,
) -> String {
    let mut cookie: Vec<(&str, &str)> = vec![
        ("date_time", "world"),
        (
            "disable_family_filter",
            safesearch_to_disable_family_filter(safesearch),
        ),
        ("disable_open_in_new_window", "0"),
        ("enable_post_method", "1"),
        ("enable_proxy_safety_suggest", "1"),
        ("enable_stay_control", "1"),
        ("instant_answers", "1"),
        ("lang_homepage", "s/device/en/"),
        ("num_of_results", "10"),
        ("suggestions", "1"),
        ("wt_unit", "celsius"),
    ];

    if let Some(lang) = language {
        cookie.push(("language", lang));
        cookie.push(("language_ui", lang));
    }
    if let Some(reg) = region {
        cookie.push(("search_results_region", reg));
    }

    cookie
        .iter()
        .map(|(k, v)| format!("{k}EEE{v}"))
        .collect::<Vec<_>>()
        .join("N1N")
}

/// Extract sgt (security gate token) from Startpage interstitial page.
/// The interstitial contains: var data = {"query":"...","sgt":"...",...};
fn extract_sgt_from_interstitial(body: &str) -> Option<String> {
    let start = body.find("var data = ")?;
    let rest = &body[start + "var data = ".len()..];
    let brace = rest.find('{')?;
    let json_start = &rest[brace..];
    let mut depth = 0;
    let mut end = 0;
    for (i, c) in json_start.chars().enumerate() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    end = i;
                    break;
                }
            }
            _ => {}
        }
    }
    let json_str = &json_start[..=end];
    let data: serde_json::Value = serde_json::from_str(json_str).ok()?;
    data.get("sgt")?.as_str().map(String::from)
}

/// Extract sc-code from Startpage homepage HTML.
fn extract_sc_code(html: &str) -> Result<String, Box<dyn Error + Send + Sync>> {
    if html.contains("/sp/captcha") {
        return Err("Startpage CAPTCHA detected".into());
    }

    let doc = Html::parse_document(html);
    let selector = Selector::parse("form#search input[name=\"sc\"]")
        .map_err(|e| format!("invalid selector: {e}"))?;

    let input = doc
        .select(&selector)
        .next()
        .ok_or("failed to extract sc-code from homepage")?;

    let value = input
        .value()
        .attr("value")
        .ok_or("failed to extract sc-code from homepage")?;

    Ok(value.to_string())
}

/// JSON structure for Startpage images response (extracted from React `AppSerpImages` props).
#[derive(Debug, Deserialize)]
struct StartpageImagesResponse {
    #[serde(default)]
    render: StartpageImagesRender,
}

#[derive(Debug, Deserialize, Default)]
struct StartpageImagesRender {
    #[serde(default)]
    presenter: StartpageImagesPresenter,
}

#[derive(Debug, Deserialize, Default)]
struct StartpageImagesPresenter {
    #[serde(default)]
    regions: StartpageImagesRegions,
}

#[derive(Debug, Deserialize, Default)]
struct StartpageImagesRegions {
    #[serde(default)]
    mainline: Vec<StartpageImagesMainlineItem>,
}

#[derive(Debug, Deserialize)]
struct StartpageImagesMainlineItem {
    #[serde(default)]
    display_type: String,
    #[serde(default)]
    results: Vec<serde_json::Value>,
}

/// Parse image result from JSON item. Returns None if altClickUrl is missing.
fn get_image_result(item: &serde_json::Value) -> Option<ImageResult> {
    let url = item.get("altClickUrl")?.as_str()?.to_string();
    if url.is_empty() {
        return None;
    }

    let thumbnail_src = item
        .get("thumbnailUrl")
        .and_then(|v| v.as_str())
        .map(|s| format!("{BASE_URL}{s}"));

    let img_src = item
        .get("rawImageUrl")
        .and_then(|v| v.as_str())
        .map(String::from)
        .unwrap_or_default();
    if img_src.is_empty() {
        return None;
    }

    let resolution = match (item.get("width"), item.get("height")) {
        (Some(w), Some(h)) => {
            let w = w.as_u64().unwrap_or(0);
            let h = h.as_u64().unwrap_or(0);
            if w > 0 && h > 0 {
                Some(format!("{w}x{h}"))
            } else {
                None
            }
        }
        _ => None,
    };

    let filesize = item.get("filesize").and_then(|v| {
        v.as_str().and_then(|s| {
            let digits: String = s.chars().filter(char::is_ascii_digit).collect();
            digits.parse::<u64>().ok().map(humanize_bytes)
        })
    });

    let title = item
        .get("title")
        .and_then(|v| v.as_str())
        .map(html_to_text)
        .unwrap_or_default();

    Some(ImageResult {
        url,
        img_src,
        thumbnail_src,
        title,
        content: None,
        source: None,
        resolution,
        img_format: item.get("format").and_then(|v| v.as_str().map(String::from)),
        filesize,
        author: None,
    })
}

/// Parse image search response by extracting JSON from React.createElement(AppSerpImages).
fn parse_image_response(body: &str) -> Result<Vec<ImageResult>, Box<dyn Error + Send + Sync>> {
    if body.contains("/sp/captcha") {
        return Err("Startpage CAPTCHA detected".into());
    }

    let json_str = extr(body, "React.createElement(UIStartpage.AppSerpImages, {", "}})")
        .ok_or("could not extract AppSerpImages JSON from response")?;

    let json_str = format!("{{{json_str}}}}}");
    let parsed: StartpageImagesResponse = serde_json::from_str(&json_str)
        .map_err(|e| format!("failed to parse Startpage images JSON: {e}"))?;

    let mut results = Vec::new();
    for mainline_item in parsed.render.presenter.regions.mainline {
        if mainline_item.display_type.contains("images") {
            for item in mainline_item.results {
                if let Some(r) = get_image_result(&item) {
                    results.push(r);
                }
            }
        }
    }

    Ok(results)
}

/// Phase of the Startpage Images state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StartpageImagesPhase {
    NeedScCode,
    NeedSearch,
    NeedSecondSearch,
    Done,
}

/// Stateful Startpage Images search provider implementing `ImageSearchProvider`.
#[derive(Debug)]
pub struct StartpageImages {
    phase: StartpageImagesPhase,
    sc_code: Option<String>,
    sgt: Option<String>,
    results: Vec<ImageResult>,
}

impl Default for StartpageImages {
    fn default() -> Self {
        Self {
            phase: StartpageImagesPhase::NeedScCode,
            sc_code: None,
            sgt: None,
            results: Vec::with_capacity(32),
        }
    }
}

impl StartpageImages {
    fn build_sc_code_request(
        _params: &SearchParams,
    ) -> Result<reqwest::Request, Box<dyn Error + Send + Sync>> {
        let url = Url::parse(&format!("{BASE_URL}/"))
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        let mut request = reqwest::Request::new(Method::GET, url);

        let headers = request.headers_mut();
        headers.insert(
            HeaderName::from_static("accept"),
            HeaderValue::from_static(
                "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
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

    fn build_search_request(
        &self,
        params: &SearchParams,
    ) -> Result<reqwest::Request, Box<dyn Error + Send + Sync>> {
        let sc_code = self
            .sc_code
            .as_ref()
            .ok_or("sc-code not available")?;

        let language = locale_to_language(&params.locale);
        let region = locale_to_region(&params.locale);

        let mut form_data = HashMap::new();
        form_data.insert("query", params.query.clone());
        form_data.insert("cat", "images".to_string());
        form_data.insert("t", "device".to_string());
        form_data.insert("sc", sc_code.clone());
        form_data.insert(
            "with_date",
            time_range_to_startpage(params.time_range).to_string(),
        );
        form_data.insert("abp", "1".to_string());
        form_data.insert("abd", "1".to_string());
        form_data.insert("abe", "1".to_string());

        if let Some(sgt) = &self.sgt {
            form_data.insert("sgt", sgt.clone());
        }

        if let Some(lang) = language {
            form_data.insert("language", lang.to_string());
            form_data.insert("lui", lang.to_string());
        }

        let body = serde_urlencoded::to_string(&form_data)
            .map_err(|e| std::io::Error::other(e.to_string()))?;

        let url = Url::parse(SEARCH_URL).map_err(|e| std::io::Error::other(e.to_string()))?;
        let mut request = reqwest::Request::new(Method::POST, url);

        let headers = request.headers_mut();
        headers.insert(
            HeaderName::from_static("content-type"),
            HeaderValue::from_static("application/x-www-form-urlencoded"),
        );
        headers.insert(
            HeaderName::from_static("origin"),
            HeaderValue::from_static(BASE_URL),
        );
        headers.insert(
            HeaderName::from_static("referer"),
            HeaderValue::from_static(BASE_URL_SLASH),
        );
        headers.insert(
            HeaderName::from_static("accept"),
            HeaderValue::from_static(
                "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            ),
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

        let cookie_value = format!(
            "preferences={}",
            build_preferences_cookie(params.safesearch, language, region)
        );
        headers.insert(
            HeaderName::from_static("cookie"),
            HeaderValue::try_from(cookie_value)
                .map_err(|e| std::io::Error::other(e.to_string()))?,
        );

        *request.body_mut() = Some(reqwest::Body::from(body));

        Ok(request)
    }
}

impl ImageSearchProvider for StartpageImages {
    fn name() -> &'static str {
        "startpage_images"
    }

    fn build_request(
        &mut self,
        params: &SearchParams,
    ) -> Result<reqwest::Request, Box<dyn Error + Send + Sync>> {
        match self.phase {
            StartpageImagesPhase::NeedScCode => Self::build_sc_code_request(params),
            StartpageImagesPhase::NeedSearch | StartpageImagesPhase::NeedSecondSearch => {
                let req = self.build_search_request(params)?;
                if self.phase == StartpageImagesPhase::NeedSecondSearch {
                    self.phase = StartpageImagesPhase::Done;
                }
                Ok(req)
            }
            StartpageImagesPhase::Done => Err("No more requests".into()),
        }
    }

    fn parse_response(&mut self, body: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        if self.phase == StartpageImagesPhase::NeedScCode {
            self.sc_code = Some(extract_sc_code(body)?);
            self.phase = StartpageImagesPhase::NeedSearch;
            return Ok(());
        }

        // First POST may return an interstitial page with sgt; submit again to get results.
        if self.phase == StartpageImagesPhase::NeedSearch {
            if let Some(sgt) = extract_sgt_from_interstitial(body) {
                self.sgt = Some(sgt);
                self.phase = StartpageImagesPhase::NeedSecondSearch;
                return Ok(());
            }
        }

        self.results = parse_image_response(body)?;
        if self.phase == StartpageImagesPhase::NeedSearch {
            self.phase = StartpageImagesPhase::Done;
        }
        Ok(())
    }

    fn results(&mut self) -> Option<Result<Vec<ImageResult>, Box<dyn Error + Send + Sync>>> {
        if self.phase != StartpageImagesPhase::Done {
            return None;
        }
        if self.results.is_empty() {
            Some(Err("No image results found".into()))
        } else {
            Some(Ok(std::mem::take(&mut self.results)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extr_app_serp_images() {
        let body = r#"React.createElement(UIStartpage.AppSerpImages, {"render":{"presenter":{"regions":{"mainline":[]}}}})"#;
        let extracted = extr(body, "React.createElement(UIStartpage.AppSerpImages, {", "}})");
        assert!(extracted.is_some());
        let s = extracted.unwrap();
        assert!(s.contains("mainline"));
    }

    #[test]
    fn test_humanize_bytes() {
        assert_eq!(humanize_bytes(500), "500 B");
        assert_eq!(humanize_bytes(1024), "1.0 KB");
        assert_eq!(humanize_bytes(1536), "1.5 KB");
        assert_eq!(humanize_bytes(1024 * 1024), "1.0 MB");
    }

    #[test]
    fn test_extract_sgt_from_interstitial() {
        let body = r#"<script>
        (function () {
          var data = {"abd": "1", "abe": "1", "abp": "1", "cat": "images", "language": "english", "lui": "english", "query": "rust logo", "sc": "kCY3F0qWLSfg20", "sgt": "1771505750T4fb08d41a3a6101bf30fa5673d53ce80497bafd4c1d0050277db55a93f94ccb0", "t": "device", "with_date": ""};
          var form = document.forms[0];
        })();
    </script>"#;
        let sgt = extract_sgt_from_interstitial(body);
        assert_eq!(
            sgt.as_deref(),
            Some("1771505750T4fb08d41a3a6101bf30fa5673d53ce80497bafd4c1d0050277db55a93f94ccb0")
        );
    }
}
