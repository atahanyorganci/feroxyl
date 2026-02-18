use quick_start::engine::{ddg, google};
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let client = reqwest::Client::new();

    // Example: DuckDuckGo search
    let ddg_response = ddg::search(
        &client,
        ddg::DuckDuckGoParams {
            query: "Rust programming".to_string(),
            page: 1,
            region: "wt-wt".to_string(),
            time_range: ddg::TimeRange::Any,
        },
    )
    .await?;

    println!("DuckDuckGo results:");
    for result in &ddg_response.results {
        println!("  - {} ({})", result.title, result.url);
        if let Some(ref content) = result.content {
            println!("    {}", content);
        }
    }
    if let Some(ref zc) = ddg_response.zero_click {
        println!("Zero-click: {} ({:?})", zc.answer, zc.url);
    }

    println!();

    // Example: Google search
    let html = google::search(
        &client,
        google::GoogleRequestParams {
            query: "Lady Gaga concert in Istanbul after 17/02/2026".to_string(),
            start: None,
        },
    )
    .await?;

    for r in google::parse_response(&html).into_iter().flatten() {
        println!("{r}");
    }

    println!("Extracted HTML length: {} bytes", html.len());
    Ok(())
}
