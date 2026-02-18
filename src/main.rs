use axum::{extract::Query, routing::get, Json, Router};
use quick_start::engine::{ddg, run_provider, SearchParams};
use std::error::Error;

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
        SearchParams {
            query: query.q,
            safesearch: quick_start::engine::Safesearch::default(),
            time_range: quick_start::engine::TimeRange::default(),
            locale: "all".to_string(),
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
