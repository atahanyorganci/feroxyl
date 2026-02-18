use axum::{extract::Query, routing::get, Json, Router};
use feroxyl::engine::{run_meta_search, Provider, RankedSearchResult, SearchParams};
use std::error::Error;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

const DEFAULT_PROVIDERS: &[Provider] = &[
    Provider::DuckDuckGo,
    Provider::Google,
    Provider::Brave,
    Provider::Startpage,
];

fn deserialize_providers<'de, D>(deserializer: D) -> Result<Vec<Provider>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de;

    #[derive(serde::Deserialize)]
    #[serde(untagged)]
    enum SingleOrSeq {
        Single(String),
        Seq(Vec<String>),
    }

    let parsed = <SingleOrSeq as serde::Deserialize>::deserialize(deserializer)?;
    let strings: Vec<String> = match parsed {
        SingleOrSeq::Single(s) => vec![s],
        SingleOrSeq::Seq(v) => v,
    };
    let mut result = Vec::new();
    for s in strings {
        for part in s.split(',').map(str::trim).filter(|p| !p.is_empty()) {
            result.push(part.parse().map_err(de::Error::custom)?);
        }
    }
    Ok(result)
}

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
    #[serde(
        default,
        rename = "provider",
        deserialize_with = "deserialize_providers"
    )]
    providers: Vec<Provider>,
}

#[tracing::instrument(skip_all, fields(query = %query, safesearch = ?safesearch, time_range = ?time_range, locale = %locale))]
async fn search(
    Query(SearchQuery {
        query,
        safesearch,
        time_range,
        locale,
        providers,
    }): Query<SearchQuery>,
) -> Json<Vec<RankedSearchResult>> {
    let params = SearchParams {
        query: query.clone(),
        safesearch,
        time_range,
        locale,
    };
    tracing::info!("Starting meta search");
    let providers: &[Provider] = if providers.is_empty() {
        DEFAULT_PROVIDERS
    } else {
        providers.as_slice()
    };
    let results = match run_meta_search(providers, &params).await {
        Ok(r) => {
            tracing::info!(count = r.len(), "Meta search completed");
            r
        }
        Err(e) => {
            tracing::error!(error = %e, "Meta search failed");
            Vec::new()
        }
    };
    Json(results)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "feroxyl=info,tower_http=info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let client = reqwest::Client::new();
    let app = Router::new()
        .route("/search", get(search))
        .layer(TraceLayer::new_for_http())
        .with_state(client);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    tracing::info!("Listening on http://127.0.0.1:3000");
    axum::serve(listener, app).await?;
    Ok(())
}

// Temporary: run brave search and print response info
// Run with: cargo run -- brave-debug rust
