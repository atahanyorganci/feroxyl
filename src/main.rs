use reqwest::Url;
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

async fn search_google(params: GoogleRequestParams) -> Result<String, Box<dyn Error>> {
    let client = reqwest::Client::new();
    let url = build_google_search_url(&params)?;

    println!("{}", url);

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

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let html = search_google(GoogleRequestParams {
        query: "Lady Gaga concert in Istanbul after 17/02/2026".to_string(),
        start: None,
    })
    .await?;

    println!("Extracted HTML length: {} bytes", html.len());
    Ok(())
}
