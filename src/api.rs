//! HTTP API routes and app factory.
//!
//! Exposed for integration testing and server setup.

use axum::{
    extract::{Path, Query},
    http::{header, StatusCode},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use reqwest::{
    header::{HeaderName, HeaderValue},
    Method, Url,
};
use tower_http::trace::TraceLayer;

use crate::engine::{
    run_image_provider, run_meta_search, BingImages, ImageResult, Provider, RankedSearchResult,
    SearchParams,
};

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
    safesearch: crate::engine::Safesearch,
    #[serde(default)]
    time_range: crate::engine::TimeRange,
    #[serde(default)]
    locale: crate::engine::Locale,
    #[serde(
        default,
        rename = "provider",
        deserialize_with = "deserialize_providers"
    )]
    providers: Vec<Provider>,
}

#[derive(serde::Deserialize)]
struct ImageSearchQuery {
    #[serde(rename = "q")]
    query: String,
    #[serde(default)]
    time_range: crate::engine::TimeRange,
    #[serde(default)]
    locale: crate::engine::Locale,
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

#[tracing::instrument(skip_all, fields(query = %query, time_range = ?time_range, locale = %locale))]
async fn search_image(
    Query(ImageSearchQuery {
        query,
        time_range,
        locale,
    }): Query<ImageSearchQuery>,
) -> Json<Vec<ImageResult>> {
    let params = SearchParams {
        query: query.clone(),
        safesearch: crate::engine::Safesearch::default(),
        time_range,
        locale,
    };
    tracing::info!("Starting image search");
    let results = match run_image_provider::<BingImages>(&params).await {
        Ok(r) => {
            tracing::info!(count = r.len(), "Image search completed");
            r
        }
        Err(e) => {
            tracing::error!(error = %e, "Image search failed");
            Vec::new()
        }
    };
    Json(results)
}

async fn scrape(Path(path): Path<String>) -> impl IntoResponse {
    let url = if path.starts_with("http://") || path.starts_with("https://") {
        path
    } else {
        format!("https://{path}")
    };
    tracing::info!("Scraping URL: {}", url);

    let mut request = reqwest::Request::new(Method::GET, Url::parse(&url).unwrap());
    let headers = request.headers_mut();
    headers.insert(
        HeaderName::from_static("accept"),
        HeaderValue::from_static("text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7"),
    );
    headers.insert(
        HeaderName::from_static("accept-language"),
        HeaderValue::from_static("en-US,en;q=0.9"),
    );
    headers.insert(
        HeaderName::from_static("accept-encoding"),
        HeaderValue::from_static("gzip, deflate, br, zstd"),
    );
    headers.insert(
        HeaderName::from_static("user-agent"),
        HeaderValue::from_static("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/144.0.0.0 Safari/537.36"),
    );
    headers.insert(
        HeaderName::from_static("cache-control"),
        HeaderValue::from_static("max-age=0"),
    );
    headers.insert(
        HeaderName::from_static("upgrade-insecure-requests"),
        HeaderValue::from_static("1"),
    );

    let client = reqwest::Client::new();

    match client.execute(request).await {
        Ok(response) => match response.text().await {
            Ok(body) => (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "text/markdown; charset=utf-8")],
                crate::scrape::html_to_markdown(&body),
            )
                .into_response(),
            Err(e) => (StatusCode::BAD_GATEWAY, e.to_string()).into_response(),
        },
        Err(e) => (StatusCode::BAD_GATEWAY, e.to_string()).into_response(),
    }
}

/// Creates the application router. Uses `()` as state since handlers create their own HTTP clients.
pub fn create_app() -> Router<()> {
    Router::new()
        .route("/search", get(search))
        .route("/search/image", get(search_image))
        .route("/scrape/*path", get(scrape))
        .layer(TraceLayer::new_for_http())
}
