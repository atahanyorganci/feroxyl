use axum::{extract::Query, routing::get, Json, Router};
use feroxyl::engine::{run_meta_search, RankedSearchResult, SearchParams};
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
) -> Json<Vec<RankedSearchResult>> {
    let params = SearchParams {
        query,
        safesearch,
        time_range,
        locale,
    };
    let results = run_meta_search(&params).await.unwrap_or_default();
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
