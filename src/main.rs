use axum::{extract::Query, routing::get, Json, Router};
use quick_start::engine::{ddg, SearchProvider};
use std::error::Error;

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

#[derive(serde::Deserialize)]
struct SearchQuery {
    q: String,
}

async fn search(
    Query(query): Query<SearchQuery>,
    axum::extract::State(client): axum::extract::State<reqwest::Client>,
) -> Json<Vec<quick_start::engine::SearchResult>> {
    let results = run_provider(
        &mut ddg::DuckDuckGo::new(),
        &client,
        ddg::DuckDuckGoParams {
            query: query.q,
            page: 1,
            region: "wt-wt".to_string(),
            time_range: ddg::TimeRange::Any,
            vqd: None,
        },
    )
    .await
    .unwrap_or_default();
    Json(results)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let client = reqwest::Client::new();
    let app = Router::new()
        .route("/search", get(search))
        .with_state(client);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    println!("Listening on http://127.0.0.1:3000");
    axum::serve(listener, app).await?;
    Ok(())
}
