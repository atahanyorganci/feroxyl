//! JSON API routes.

use axum::{
    Json, Router,
    extract::{Path, Query},
    http::{StatusCode, header},
    response::IntoResponse,
    routing::get,
};
use reqwest::{
    Method, Url,
    header::{HeaderName, HeaderValue},
};

use super::{DEFAULT_IMAGE_PROVIDERS, DEFAULT_PROVIDERS, ImageSearchQuery, SearchQuery};
use crate::engine::{
    ImageProvider, Provider, RankedImageResult, RankedSearchResult, SearchParams,
    run_meta_image_search, run_meta_search,
};

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

#[tracing::instrument(skip_all, fields(query = %query, safesearch = ?safesearch, time_range = ?time_range, locale = %locale))]
async fn search_image(
    Query(ImageSearchQuery {
        query,
        safesearch,
        time_range,
        locale,
        providers,
    }): Query<ImageSearchQuery>,
) -> Json<Vec<RankedImageResult>> {
    let params = SearchParams {
        query: query.clone(),
        safesearch,
        time_range,
        locale,
    };
    tracing::info!("Starting image search");
    let providers: &[ImageProvider] = if providers.is_empty() {
        DEFAULT_IMAGE_PROVIDERS
    } else {
        providers.as_slice()
    };
    let results = match run_meta_image_search(providers, &params).await {
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

pub fn routes() -> Router<()> {
    Router::new()
        .route("/search", get(search))
        .route("/search/image", get(search_image))
        .route("/scrape/*path", get(scrape))
}
