use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use feroxyl::engine::{ddg, run_provider, SearchParams, SearchResult};
use std::error::Error;

#[derive(serde::Deserialize)]
struct SearchQuery {
    #[serde(rename = "q")]
    query: String,
    #[serde(default)]
    safesearch: feroxyl::engine::Safesearch,
    #[serde(default)]
    time_range: feroxyl::engine::TimeRange,
    #[serde(default)]
    locale: feroxyl::engine::Locale,
}

async fn search(
    Query(SearchQuery {
        query,
        safesearch,
        time_range,
        locale,
    }): Query<SearchQuery>,
    State(client): State<reqwest::Client>,
) -> Json<Vec<SearchResult>> {
    let results = run_provider(
        &mut ddg::DuckDuckGo::new(),
        &client,
        SearchParams {
            query,
            safesearch,
            time_range,
            locale,
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
