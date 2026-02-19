//! Unsplash image search engine
//!
//! Port of `SearXNG`'s `unsplash.py` engine.
//! Uses the internal API at <https://unsplash.com/napi/search/photos>

use std::error::Error;

use reqwest::{
    Method, Url,
    header::{HeaderName, HeaderValue},
};
use serde::Deserialize;

use crate::engine::{ImageResult, ImageSearchProvider, SearchParams};

const BASE_URL: &str = "https://unsplash.com/napi/search/photos";
const PAGE_SIZE: u32 = 20;

/// Remove `ixid` from URL query params (SearXNG compatibility).
fn clean_url(url_str: &str) -> String {
    let Ok(mut url) = Url::parse(url_str) else {
        return url_str.to_string();
    };
    let pairs: Vec<(String, String)> = url
        .query_pairs()
        .filter(|(k, _)| k != "ixid")
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

/// Unsplash API response structure.
#[derive(Debug, Deserialize)]
struct UnsplashResponse {
    #[serde(default)]
    results: Vec<UnsplashResult>,
}

#[derive(Debug, Deserialize)]
struct UnsplashResult {
    #[serde(default)]
    links: UnsplashLinks,
    #[serde(default)]
    urls: UnsplashUrls,
    #[serde(default)]
    alt_description: Option<String>,
    #[serde(default)]
    description: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct UnsplashLinks {
    #[serde(default)]
    html: String,
}

#[derive(Debug, Deserialize, Default)]
struct UnsplashUrls {
    #[serde(default)]
    thumb: String,
    #[serde(default)]
    regular: String,
}

/// Parse Unsplash JSON response into `ImageResult`s.
fn parse_response(body: &str) -> Result<Vec<ImageResult>, Box<dyn Error + Send + Sync>> {
    let data: UnsplashResponse = serde_json::from_str(body)
        .map_err(|e| std::io::Error::other(format!("Invalid JSON: {e}")))?;

    let results: Vec<ImageResult> = data
        .results
        .into_iter()
        .filter_map(|r| {
            let url = clean_url(&r.links.html);
            if url.is_empty() {
                return None;
            }
            let img_src = clean_url(&r.urls.regular);
            if img_src.is_empty() {
                return None;
            }
            let thumbnail_src = if r.urls.thumb.is_empty() {
                None
            } else {
                Some(clean_url(&r.urls.thumb))
            };
            let title = r
                .alt_description
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "unknown".to_string());
            let content = r.description.filter(|s| !s.is_empty());

            Some(ImageResult {
                url,
                img_src,
                thumbnail_src,
                title,
                content,
                source: Some("Unsplash".to_string()),
                resolution: None,
                img_format: None,
                filesize: None,
                author: None,
            })
        })
        .collect();

    Ok(results)
}

/// Stateful Unsplash image search provider implementing `ImageSearchProvider`.
#[derive(Debug)]
pub struct Unsplash {
    results: Vec<ImageResult>,
}

impl Default for Unsplash {
    fn default() -> Self {
        Self {
            results: Vec::with_capacity(PAGE_SIZE as usize),
        }
    }
}

impl ImageSearchProvider for Unsplash {
    fn name() -> &'static str {
        "unsplash"
    }

    fn build_request(
        &mut self,
        params: &SearchParams,
    ) -> Result<reqwest::Request, Box<dyn Error + Send + Sync>> {
        let page = 1u32;

        let mut url = Url::parse(BASE_URL).map_err(|e| std::io::Error::other(e.to_string()))?;
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("query", &params.query);
            pairs.append_pair("page", &page.to_string());
            pairs.append_pair("per_page", &PAGE_SIZE.to_string());
        }

        let mut request = reqwest::Request::new(Method::GET, url);

        let headers = request.headers_mut();
        headers.insert(
            HeaderName::from_static("accept"),
            HeaderValue::from_static("application/json"),
        );
        headers.insert(
            HeaderName::from_static("referer"),
            HeaderValue::from_static("https://unsplash.com/"),
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
    fn test_clean_url_removes_ixid() {
        let url = "https://unsplash.com/s/photos/cat?ixid=abc123&foo=bar";
        assert_eq!(clean_url(url), "https://unsplash.com/s/photos/cat?foo=bar");
    }

    #[test]
    fn test_clean_url_preserves_other_params() {
        let url = "https://example.com?a=1&b=2";
        assert_eq!(clean_url(url), "https://example.com/?a=1&b=2");
    }

    #[test]
    fn test_parse_empty_results() {
        let body = r#"{"results":[]}"#;
        let results = parse_response(body).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_parse_single_result() {
        let body = r#"{
            "results": [{
                "links": {"html": "https://unsplash.com/photos/abc"},
                "urls": {"thumb": "https://images.unsplash.com/thumb", "regular": "https://images.unsplash.com/regular"},
                "alt_description": "A cat",
                "description": "Cute cat photo"
            }]
        }"#;
        let results = parse_response(body).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "A cat");
        assert_eq!(results[0].content.as_deref(), Some("Cute cat photo"));
        assert_eq!(results[0].img_src, "https://images.unsplash.com/regular");
        assert_eq!(results[0].source.as_deref(), Some("Unsplash"));
    }
}
