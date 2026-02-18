use quick_start::engine::{ddg, google, SearchProvider};
use std::error::Error;

async fn run_provider<P: SearchProvider>(
    provider: P,
    client: &reqwest::Client,
    params: P::Params,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let req = provider.build_request(params)?;
    let response = client.execute(req).await?;
    let body = response.text().await?;
    let results = provider.parse_response(&body)?;
    for r in results {
        println!("  - {} ({})", r.title, r.url);
        if let Some(ref content) = r.content {
            println!("    {}", content);
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let client = reqwest::Client::new();

    println!("DuckDuckGo results:");
    let vqd = ddg::get_vqd(&client, "Rust programming", "wt-wt")
        .await
        .ok()
        .flatten();
    run_provider(
        ddg::DuckDuckGo,
        &client,
        ddg::DuckDuckGoParams {
            query: "Rust programming".to_string(),
            page: 1,
            region: "wt-wt".to_string(),
            time_range: ddg::TimeRange::Any,
            vqd,
        },
    )
    .await?;

    println!("\nGoogle results:");
    run_provider(
        google::Google,
        &client,
        google::GoogleRequestParams {
            query: "Lady Gaga concert in Istanbul after 17/02/2026".to_string(),
            start: None,
        },
    )
    .await?;

    Ok(())
}
