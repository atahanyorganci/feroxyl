use quick_start::engine::{ddg, google, SearchProvider};
use std::error::Error;

fn print_results(results: &[quick_start::engine::SearchResult]) {
    for r in results {
        println!("  - {} ({})", r.title, r.url);
        if let Some(ref content) = r.content {
            println!("    {}", content);
        }
    }
}

async fn run_provider<P: SearchProvider>(
    provider: &mut P,
    client: &reqwest::Client,
    params: P::Params,
) -> Result<Vec<quick_start::engine::SearchResult>, Box<dyn Error + Send + Sync>> {
    let mut all = Vec::new();
    let mut params = Some(params);

    loop {
        let req = match provider.build_request(params.take()) {
            Ok(Some(r)) => r,
            Ok(None) => break,
            Err(e) => return Err(e),
        };
        let response = client.execute(req).await?;
        let body = response.text().await?;
        provider.parse_response(&body)?;

        while let Some(r) = provider.results() {
            match r {
                Ok(sr) => all.extend(sr),
                Err(e) => return Err(e),
            }
        }
    }
    Ok(all)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let client = reqwest::Client::new();

    println!("DuckDuckGo results:");
    let mut ddg_provider = ddg::DuckDuckGo::new();
    let results = run_provider(
        &mut ddg_provider,
        &client,
        ddg::DuckDuckGoParams {
            query: "Rust programming".to_string(),
            page: 1,
            region: "wt-wt".to_string(),
            time_range: ddg::TimeRange::Any,
            vqd: None,
        },
    )
    .await?;
    print_results(&results);

    println!("\nGoogle results:");
    let mut google_provider = google::Google::new();
    let results = run_provider(
        &mut google_provider,
        &client,
        google::GoogleRequestParams {
            query: "Lady Gaga concert in Istanbul after 17/02/2026".to_string(),
            start: None,
        },
    )
    .await?;
    print_results(&results);

    Ok(())
}
