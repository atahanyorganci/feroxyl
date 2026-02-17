use core::fmt;
use reqwest::Url;
use scraper::{ElementRef, Html, Selector};
use std::error::Error;

#[derive(Debug, Clone)]
struct GoogleRequestParams {
    query: String,
    start: Option<u32>,
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

async fn send_request(params: GoogleRequestParams) -> Result<String, Box<dyn Error>> {
    let client = reqwest::Client::new();
    let url = build_google_search_url(&params)?;

    let response = client
        .get(url)
        .header("Accept", "*/*")
        .header("Sec-Fetch-Dest", "empty")
        .header("Sec-Fetch-Mode", "cors")
        .header("Sec-Fetch-Site", "same-origin")
        .header("Sec-Fetch-User", "?1")
        .header("Sec-GPC", "1")
        .header(
            "User-Agent",
            "Mozilla/5.0 (iPhone; CPU iPhone OS 18_6_2 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) GSA/399.2.845414227 Mobile/15E148 Safari/604.1",
        )
        .header("Cookie", "CONSENT=YES+")
        .send()
        .await?;

    let mut html = response.text().await?;

    let start_index = html.find("<div").ok_or("No <div> found")?;
    html = html[start_index..].to_string();

    let end_index = html.rfind("</div>").ok_or("No </div> found")?;
    html = html[..end_index].to_string();

    Ok(html)
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
            content.push_str(&text);
        }
    }
    if content.is_empty() {
        None
    } else {
        Some(content)
    }
}

fn extract_url(element: ElementRef) -> Result<String, Box<dyn Error>> {
    let href = element.value().attr("href").unwrap();
    let url = format!("https://www.google.com{}", href);
    let url = Url::parse(&url).unwrap();
    let url = url
        .query_pairs()
        .find(|(key, _)| key == "q")
        .unwrap()
        .1
        .to_string();
    Ok(url)
}

#[derive(Debug, Clone)]
struct GoogleResult {
    title: String,
    url: String,
    content: Option<String>,
}

impl fmt::Display for GoogleResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Result {{ title: {}, url: {}", self.title, self.url)?;
        if let Some(content) = &self.content {
            write!(f, ", content: {}", content)?;
        }
        write!(f, " }}")
    }
}

fn parse_google_result(element: ElementRef) -> Result<GoogleResult, Box<dyn Error>> {
    let title = extract_title(element)?;
    let url = extract_url(element)?;
    let content = extract_content(element);
    Ok(GoogleResult {
        title,
        url,
        content,
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let html = send_request(GoogleRequestParams {
        query: "Lady Gaga concert in Istanbul after 17/02/2026".to_string(),
        start: None,
    })
    .await?;

    let document = Html::parse_fragment(&html);
    let selector = Selector::parse("div.MjjYud").unwrap();
    for element in document.select(&selector) {
        let link_selector = Selector::parse("a[href*='/url?q=']").unwrap();
        if let Some(link) = element.select(&link_selector).next() {
            if let Ok(result) = parse_google_result(link) {
                println!("{result}");
            }
        }
    }

    println!("Extracted HTML length: {} bytes", html.len());
    Ok(())
}
