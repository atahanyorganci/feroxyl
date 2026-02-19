//! Google Images search engine
//!
//! Port of `SearXNG`'s `google_images.py` engine.
//! Uses the internal Google API (JSON format) at <https://www.google.com/search?tbm=isch>

use std::error::Error;

use reqwest::{
    header::{HeaderName, HeaderValue},
    Method, Url,
};

use crate::engine::{
    ImageResult, ImageSearchProvider, Locale, Safesearch, SearchParams, TimeRange,
};

const BASE_URL: &str = "https://www.google.com/search";

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

/// Safesearch to Google Images safe param (`filter_mapping`: 0→images, 1→active, 2→active)
fn safesearch_to_google_images(s: Safesearch) -> &'static str {
    match s {
        Safesearch::Off => "images",
        Safesearch::Moderate | Safesearch::Strict => "active",
    }
}

/// Google hl (interface language) param
fn locale_to_google_hl(locale: &Locale) -> &str {
    match locale {
        Locale::All | Locale::EnUS => "en-US",
        Locale::EnGB => "en-GB",
        Locale::TrTR => "tr",
        Locale::Other(s) => s.as_str(),
    }
}

/// Google lr (language restriction) param
fn locale_to_google_lr(locale: &Locale) -> Option<&'static str> {
    match locale {
        Locale::EnUS | Locale::EnGB => Some("lang_en"),
        Locale::TrTR => Some("lang_tr"),
        Locale::All | Locale::Other(_) => None,
    }
}

/// Google cr (country restriction) param
fn locale_to_google_cr(locale: &Locale) -> Option<&'static str> {
    match locale {
        Locale::EnUS => Some("countryUS"),
        Locale::EnGB => Some("countryGB"),
        Locale::TrTR => Some("countryTR"),
        Locale::All | Locale::Other(_) => None,
    }
}

/// Country code for User-Agent (e.g. "US", "GB")
fn locale_to_country(locale: &Locale) -> &'static str {
    match locale {
        Locale::EnGB => "GB",
        Locale::TrTR => "TR",
        Locale::EnUS | Locale::All | Locale::Other(_) => "US",
    }
}

fn build_source(
    result_obj: &serde_json::Map<String, serde_json::Value>,
    gsa: &serde_json::Map<String, serde_json::Value>,
) -> (Option<String>, Option<String>) {
    let site_title = result_obj
        .get("site_title")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    let mut source = site_title.to_string();
    if let Some(iptc) = result_obj.get("iptc").and_then(|i| i.as_object()) {
        if let Some(cn) = iptc.get("copyright_notice").and_then(serde_json::Value::as_str) {
            if !cn.is_empty() {
                source.push_str(" | ");
                source.push_str(cn);
            }
        }
    }
    let freshness = result_obj
        .get("freshness_date")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    if !freshness.is_empty() {
        source.push_str(" | ");
        source.push_str(freshness);
    }
    let file_size = gsa
        .get("file_size")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    if let Some(ref fs) = &file_size {
        if !fs.is_empty() {
            source.push_str(" (");
            source.push_str(fs);
            source.push(')');
        }
    }
    let source = if source.is_empty() {
        None
    } else {
        Some(source)
    };
    (source, file_size)
}

fn parse_image_item(
    item: &serde_json::Map<String, serde_json::Value>,
) -> Option<ImageResult> {
    let empty_map = serde_json::Map::new();
    let result_obj = item.get("result").and_then(|r| r.as_object()).unwrap_or(&empty_map);
    let text_in_grid = item.get("text_in_grid").and_then(|t| t.as_object()).unwrap_or(&empty_map);
    let original_image = item.get("original_image").and_then(|o| o.as_object()).unwrap_or(&empty_map);
    let thumbnail = item.get("thumbnail").and_then(|t| t.as_object()).unwrap_or(&empty_map);
    let gsa = item.get("gsa").and_then(|g| g.as_object()).unwrap_or(&empty_map);

    let referrer_url = result_obj
        .get("referrer_url")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .to_string();
    let page_title = result_obj
        .get("page_title")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .to_string();
    let snippet = text_in_grid
        .get("snippet")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .to_string();
    let img_url = original_image
        .get("url")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .to_string();
    let width = original_image
        .get("width")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let width = u32::try_from(width).unwrap_or(0);
    let height = original_image
        .get("height")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let height = u32::try_from(height).unwrap_or(0);
    let thumb_url = thumbnail
        .get("url")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .to_string();

    if referrer_url.is_empty() || img_url.is_empty() {
        return None;
    }

    let (source, file_size) = build_source(result_obj, gsa);

    let author = result_obj
        .get("iptc")
        .and_then(|i| i.get("creator"))
        .and_then(|c| c.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join(", "))
        .filter(|s| !s.is_empty());

    let resolution = if width > 0 && height > 0 {
        Some(format!("{width} x {height}"))
    } else {
        None
    };

    Some(ImageResult {
        url: referrer_url,
        img_src: img_url,
        thumbnail_src: if thumb_url.is_empty() {
            None
        } else {
            Some(thumb_url)
        },
        title: page_title,
        content: if snippet.is_empty() {
            None
        } else {
            Some(snippet)
        },
        source,
        resolution,
        img_format: None,
        filesize: file_size,
        author,
    })
}

/// Parse Google Images JSON response into `ImageResult`s.
fn parse_response(body: &str) -> Result<Vec<ImageResult>, Box<dyn Error + Send + Sync>> {
    let json_start = body
        .find(r#"{"ischj":"#)
        .or_else(|| body.find(r#"{"ischj": "#))
        .or_else(|| body.find("{\"ischj\":"))
        .ok_or_else(|| {
            std::io::Error::other("Google Images: no {\"ischj\": marker found in response")
        })?;

    let json_str = &body[json_start..];
    let json_value: serde_json::Value = serde_json::from_str(json_str)
        .map_err(|e| std::io::Error::other(format!("Google Images JSON parse error: {e}")))?;

    let ischj = json_value
        .get("ischj")
        .ok_or_else(|| std::io::Error::other("Google Images: no ischj key in JSON"))?;

    let empty: Vec<serde_json::Value> = Vec::new();
    let metadata = ischj
        .get("metadata")
        .and_then(|m| m.as_array())
        .unwrap_or(&empty);

    let mut results = Vec::new();
    for item in metadata {
        let Some(item) = item.as_object() else {
            continue;
        };

        if let Some(result) = parse_image_item(item) {
            results.push(result);
        }
    }

    Ok(results)
}

/// Stateful Google Images search provider implementing `ImageSearchProvider`.
#[derive(Debug)]
pub struct GoogleImages {
    results: Vec<ImageResult>,
}

impl Default for GoogleImages {
    fn default() -> Self {
        Self {
            results: Vec::with_capacity(100),
        }
    }
}

impl ImageSearchProvider for GoogleImages {
    fn name() -> &'static str {
        "google_images"
    }

    fn build_request(
        &mut self,
        params: &SearchParams,
    ) -> Result<reqwest::Request, Box<dyn Error + Send + Sync>> {
        let pageno = 1u32;
        let ijn = pageno.saturating_sub(1);

        let mut url = Url::parse(BASE_URL).map_err(|e| std::io::Error::other(e.to_string()))?;
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("q", &params.query);
            pairs.append_pair("tbm", "isch");
            pairs.append_pair("asearch", "isch");
            pairs.append_pair("async", &format!("_fmt:json,p:1,ijn:{ijn}"));
            pairs.append_pair("hl", locale_to_google_hl(&params.locale));
            pairs.append_pair("ie", "utf8");
            pairs.append_pair("oe", "utf8");
            if let Some(lr) = locale_to_google_lr(&params.locale) {
                pairs.append_pair("lr", lr);
            }
            if let Some(cr) = locale_to_google_cr(&params.locale) {
                pairs.append_pair("cr", cr);
            }
            if let Some(tbs) = time_range_to_google_tbs(params.time_range) {
                pairs.append_pair("tbs", &format!("qdr:{tbs}"));
            }
            pairs.append_pair("safe", safesearch_to_google_images(params.safesearch));
        }

        let country = locale_to_country(&params.locale);

        let mut request = reqwest::Request::new(Method::GET, url);

        let headers = request.headers_mut();
        headers.insert(
            HeaderName::from_static("accept"),
            HeaderValue::from_static("*/*"),
        );
        headers.insert(
            HeaderName::from_static("user-agent"),
            HeaderValue::try_from(format!(
                "NSTN/3.60.474802233.release Dalvik/2.1.0 (Linux; U; Android 12; {country}) gzip"
            ))
            .map_err(|e| std::io::Error::other(e.to_string()))?,
        );
        headers.insert(
            HeaderName::from_static("cookie"),
            HeaderValue::from_static("CONSENT=YES+"),
        );
        headers.insert(
            HeaderName::from_static("accept-language"),
            HeaderValue::from_static("en-US,en;q=0.9"),
        );

        Ok(request)
    }

    fn parse_response(&mut self, body: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.results = parse_response(body)?;
        Ok(())
    }

    fn results(&mut self) -> Option<Result<Vec<ImageResult>, Box<dyn Error + Send + Sync>>> {
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
    fn test_time_range_to_google_tbs() {
        assert_eq!(time_range_to_google_tbs(TimeRange::Any), None);
        assert_eq!(time_range_to_google_tbs(TimeRange::Day), Some("d"));
        assert_eq!(time_range_to_google_tbs(TimeRange::Week), Some("w"));
        assert_eq!(time_range_to_google_tbs(TimeRange::Month), Some("m"));
        assert_eq!(time_range_to_google_tbs(TimeRange::Year), Some("y"));
    }

    #[test]
    fn test_safesearch_to_google_images() {
        assert_eq!(safesearch_to_google_images(Safesearch::Off), "images");
        assert_eq!(safesearch_to_google_images(Safesearch::Moderate), "active");
        assert_eq!(safesearch_to_google_images(Safesearch::Strict), "active");
    }

    #[test]
    fn test_locale_to_country() {
        assert_eq!(locale_to_country(&Locale::All), "US");
        assert_eq!(locale_to_country(&Locale::EnUS), "US");
        assert_eq!(locale_to_country(&Locale::EnGB), "GB");
        assert_eq!(locale_to_country(&Locale::TrTR), "TR");
    }
}
