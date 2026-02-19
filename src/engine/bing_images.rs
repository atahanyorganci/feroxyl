//! Bing Images search engine
//!
//! Port of `SearXNG`'s `bing_images.py` engine.
//! Uses the HTML API at <https://www.bing.com/images/async>

use reqwest::Method;
use reqwest::Url;
use reqwest::header::{HeaderName, HeaderValue};
use scraper::{Html, Selector};
use std::error::Error;

use crate::engine::{ImageResult, ImageSearchProvider, Locale, SearchParams, TimeRange};

const BASE_URL: &str = "https://www.bing.com/images/async";

/// Time range to Bing Images filter (minutes).
/// Maps to qft=filterui:age-lt{minutes}
fn time_range_to_minutes(tr: TimeRange) -> Option<u32> {
    match tr {
        TimeRange::Any => None,
        TimeRange::Day => Some(60 * 24),
        TimeRange::Week => Some(60 * 24 * 7),
        TimeRange::Month => Some(60 * 24 * 31),
        TimeRange::Year => Some(60 * 24 * 365),
    }
}

/// Bing engine region from Locale (market code).
fn locale_to_region(locale: &Locale) -> &'static str {
    match locale {
        Locale::All | Locale::EnUS | Locale::Other(_) => "en-us",
        Locale::EnGB => "en-gb",
        Locale::TrTR => "tr-tr",
    }
}

/// Metadata JSON embedded in a.iusc @m attribute.
#[derive(Debug, serde::Deserialize)]
struct IuscMetadata {
    purl: String,
    turl: String,
    murl: String,
    #[serde(default)]
    desc: Option<String>,
}

/// Parse Bing Images HTML response into `ImageResults`.
fn parse_response(html: &str) -> Result<Vec<ImageResult>, Box<dyn Error + Send + Sync>> {
    let doc = Html::parse_document(html);

    let list_selector = Selector::parse("ul[class*=\"dgControl_list\"] > li").unwrap();
    let iusc_selector = Selector::parse("a.iusc").unwrap();
    let infnmpt_selector = Selector::parse("div.infnmpt a").unwrap();
    let imgpt_div_selector = Selector::parse("div.imgpt > div").unwrap();
    let lnkw_selector = Selector::parse("div.imgpt div.lnkw a").unwrap();

    let mut results = Vec::new();

    for li in doc.select(&list_selector) {
        let metadata_attr = li
            .select(&iusc_selector)
            .next()
            .and_then(|a| a.value().attr("m"));

        let Some(metadata_json) = metadata_attr else {
            continue;
        };

        let metadata: IuscMetadata = serde_json::from_str(metadata_json)
            .map_err(|e| std::io::Error::other(format!("Invalid iusc metadata: {e}")))?;

        let title = li
            .select(&infnmpt_selector)
            .map(|a| a.text().collect::<String>())
            .collect::<String>()
            .trim()
            .to_string();

        // Python: ' '.join(div.imgpt/div/span/text()).strip().split(" · ")
        let img_format_raw: String = li
            .select(&imgpt_div_selector)
            .next()
            .map(|div| div.text().collect::<String>())
            .unwrap_or_default()
            .trim()
            .to_string();
        let img_format_parts: Vec<&str> = img_format_raw.split(" · ").map(str::trim).collect();
        let resolution = img_format_parts.first().and_then(|s| {
            if s.is_empty() {
                None
            } else {
                Some((*s).to_string())
            }
        });
        let img_format = img_format_parts.get(1).and_then(|s| {
            if s.is_empty() {
                None
            } else {
                Some((*s).to_string())
            }
        });

        let source = li
            .select(&lnkw_selector)
            .map(|a| a.text().collect::<String>())
            .collect::<String>()
            .trim()
            .to_string();
        let source = if source.is_empty() {
            None
        } else {
            Some(source)
        };

        // Skip results with missing required URLs (Bing sometimes returns incomplete entries)
        if metadata.purl.is_empty() || metadata.murl.is_empty() {
            continue;
        }

        results.push(ImageResult {
            url: metadata.purl,
            img_src: metadata.murl,
            thumbnail_src: Some(metadata.turl),
            title: if title.is_empty() {
                metadata.desc.clone().unwrap_or_default()
            } else {
                title
            },
            content: metadata.desc,
            source,
            resolution,
            img_format,
            filesize: None,
            author: None,
        });
    }

    Ok(results)
}

/// Stateful Bing Images search provider implementing `ImageSearchProvider`.
#[derive(Debug)]
pub struct BingImages {
    results: Vec<ImageResult>,
}

impl Default for BingImages {
    fn default() -> Self {
        Self {
            results: Vec::with_capacity(35),
        }
    }
}

impl ImageSearchProvider for BingImages {
    fn name() -> &'static str {
        "bing_images"
    }

    fn build_request(
        &mut self,
        params: &SearchParams,
    ) -> Result<reqwest::Request, Box<dyn Error + Send + Sync>> {
        let page = 1u32;
        let first = (page.saturating_sub(1)) * 35 + 1;

        let mut url = Url::parse(BASE_URL).map_err(|e| std::io::Error::other(e.to_string()))?;
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("q", &params.query);
            pairs.append_pair("async", "1");
            pairs.append_pair("first", &first.to_string());
            pairs.append_pair("count", "35");

            if let Some(minutes) = time_range_to_minutes(params.time_range) {
                pairs.append_pair("qft", &format!("filterui:age-lt{minutes}"));
            }
        }

        let region = locale_to_region(&params.locale);
        let language = region;

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
            HeaderName::from_static("user-agent"),
            HeaderValue::from_static(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
            ),
        );

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
    fn test_time_range_to_minutes() {
        assert_eq!(time_range_to_minutes(TimeRange::Any), None);
        assert_eq!(time_range_to_minutes(TimeRange::Day), Some(60 * 24));
        assert_eq!(time_range_to_minutes(TimeRange::Week), Some(60 * 24 * 7));
        assert_eq!(time_range_to_minutes(TimeRange::Month), Some(60 * 24 * 31));
        assert_eq!(time_range_to_minutes(TimeRange::Year), Some(60 * 24 * 365));
    }

    #[test]
    fn test_locale_to_region() {
        assert_eq!(locale_to_region(&Locale::All), "en-us");
        assert_eq!(locale_to_region(&Locale::EnUS), "en-us");
        assert_eq!(locale_to_region(&Locale::EnGB), "en-gb");
        assert_eq!(locale_to_region(&Locale::TrTR), "tr-tr");
    }
}
