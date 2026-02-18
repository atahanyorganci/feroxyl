//! Google search engine

use reqwest::header::{HeaderName, HeaderValue};
use reqwest::Method;
use reqwest::Url;
use scraper::{ElementRef, Html, Selector};
use std::collections::VecDeque;
use std::error::Error;

use crate::engine::{SearchProvider, SearchResult};

/// Parameters for a Google search request
#[derive(Debug, Clone, Default)]
pub struct GoogleRequestParams {
    pub query: String,
    pub start: Option<u32>,
}

fn build_google_search_url(params: &GoogleRequestParams) -> Result<Url, Box<dyn Error>> {
    let mut url = Url::parse("https://www.google.com/search")?;
    url.query_pairs_mut()
        .append_pair("q", &params.query)
        .append_pair("hl", "en-US")
        .append_pair("lr", "lang_en")
        .append_pair("cr", "countryUS")
        .append_pair("ie", "utf8")
        .append_pair("oe", "utf8")
        .append_pair("filter", "0")
        .append_pair("start", &params.start.unwrap_or(0).to_string())
        .append_pair("asearch", "arc")
        .append_pair(
            "async",
            "arc_id:srp_OYU6IpFlzDNEiO26LbU1F7p_100,use_ac:true,_fmt:prog",
        );
    Ok(url)
}

fn extract_title(element: ElementRef) -> Result<String, Box<dyn Error>> {
    let selector = Selector::parse("div[role='link']").unwrap();
    if let Some(element) = element.select(&selector).next() {
        return Ok(element.text().collect::<String>());
    }
    let selector = Selector::parse("div[role*='link']").unwrap();
    if let Some(element) = element.select(&selector).next() {
        return Ok(element.text().collect::<String>());
    }
    let selector = Selector::parse("[data-snf='GuLy6c']").unwrap();
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

fn extract_url(root: ElementRef) -> Result<String, Box<dyn Error>> {
    let link_selector = Selector::parse("a[href*='/url?q=']").unwrap();
    if let Some(link) = root.select(&link_selector).next() {
        let href = link.value().attr("href").unwrap();
        let url = format!("https://www.google.com{}", href);
        let url = Url::parse(&url).unwrap();
        let url = url
            .query_pairs()
            .find(|(key, _)| key == "q")
            .unwrap()
            .1
            .to_string();
        Ok(url)
    } else {
        Err("No link found".into())
    }
}

fn parse_google_result(element: ElementRef) -> Result<SearchResult, Box<dyn Error>> {
    let title = extract_title(element)?;
    let url = extract_url(element)?;
    let content = extract_content(element);
    Ok(SearchResult {
        title,
        url,
        content,
    })
}

/// Parses Google search HTML and returns results
fn parse_response(html: &str) -> Vec<Result<SearchResult, Box<dyn Error>>> {
    let document = Html::parse_fragment(html);
    let selector = Selector::parse("div.MjjYud").unwrap();
    document
        .select(&selector)
        .map(parse_google_result)
        .collect()
}

/// Stateful Google search provider implementing SearchProvider.
#[derive(Debug, Default)]
pub struct Google {
    params: Option<GoogleRequestParams>,
    result_queue: VecDeque<SearchResult>,
    request_sent: bool,
}

impl Google {
    pub fn new() -> Self {
        Self::default()
    }
}

impl SearchProvider for Google {
    type Params = GoogleRequestParams;

    fn build_request(
        &mut self,
        params: Option<Self::Params>,
    ) -> Result<Option<reqwest::Request>, Box<dyn Error + Send + Sync>> {
        if self.request_sent {
            return Ok(None);
        }

        let params = params.or_else(|| self.params.clone());
        let params = match params {
            Some(p) => p,
            None => return Ok(None),
        };

        self.params = Some(params.clone());
        self.request_sent = true;

        let url =
            build_google_search_url(&params).map_err(|e| std::io::Error::other(e.to_string()))?;
        let mut request = reqwest::Request::new(Method::GET, url);
        let headers = request.headers_mut();
        headers.insert(
            HeaderName::from_static("accept"),
            HeaderValue::from_static("*/*"),
        );
        headers.insert(
            HeaderName::from_static("sec-fetch-dest"),
            HeaderValue::from_static("empty"),
        );
        headers.insert(
            HeaderName::from_static("sec-fetch-mode"),
            HeaderValue::from_static("cors"),
        );
        headers.insert(
            HeaderName::from_static("sec-fetch-site"),
            HeaderValue::from_static("same-origin"),
        );
        headers.insert(
            HeaderName::from_static("sec-fetch-user"),
            HeaderValue::from_static("?1"),
        );
        headers.insert(
            HeaderName::from_static("sec-gpc"),
            HeaderValue::from_static("1"),
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
        Ok(Some(request))
    }

    fn parse_response(&mut self, body: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut html = body.to_string();
        let start_index = html.find("<div").ok_or("No <div> found")?;
        html = html[start_index..].to_string();
        let end_index = html.rfind("</div>").ok_or("No </div> found")?;
        html = html[..end_index].to_string();

        for r in parse_response(&html).into_iter().filter_map(|r| r.ok()) {
            self.result_queue.push_back(r);
        }
        Ok(())
    }

    fn results(
        &mut self,
    ) -> Option<Result<crate::engine::SearchResult, Box<dyn Error + Send + Sync>>> {
        self.result_queue.pop_front().map(Ok)
    }
}
