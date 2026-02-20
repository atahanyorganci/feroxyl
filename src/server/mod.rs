//! HTTP server: app factory and shared types.
//!
//! Exposed for integration testing and server setup.

mod api;
mod view;

pub use api::routes as api_routes;
use axum::Router;
use tower_http::trace::TraceLayer;
pub use view::routes as view_routes;

use crate::engine::{ImageProvider, Provider};

pub(crate) const DEFAULT_PROVIDERS: &[Provider] = &[
    Provider::DuckDuckGo,
    Provider::Google,
    Provider::Brave,
    Provider::Startpage,
];

pub(crate) const DEFAULT_IMAGE_PROVIDERS: &[ImageProvider] = &[
    ImageProvider::BingImages,
    ImageProvider::GoogleImages,
    ImageProvider::StartpageImages,
    ImageProvider::Unsplash,
];

pub(crate) fn deserialize_providers<'de, D>(deserializer: D) -> Result<Vec<Provider>, D::Error>
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

pub(crate) fn deserialize_image_providers<'de, D>(
    deserializer: D,
) -> Result<Vec<ImageProvider>, D::Error>
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
pub(crate) struct SearchQuery {
    #[serde(rename = "q")]
    pub query: String,
    #[serde(default)]
    pub safesearch: crate::engine::Safesearch,
    #[serde(default)]
    pub time_range: crate::engine::TimeRange,
    #[serde(default)]
    pub locale: crate::engine::Locale,
    #[serde(
        default,
        rename = "provider",
        deserialize_with = "deserialize_providers"
    )]
    pub providers: Vec<Provider>,
}

#[derive(serde::Deserialize)]
pub(crate) struct ImageSearchQuery {
    #[serde(rename = "q")]
    pub query: String,
    #[serde(default)]
    pub safesearch: crate::engine::Safesearch,
    #[serde(default)]
    pub time_range: crate::engine::TimeRange,
    #[serde(default)]
    pub locale: crate::engine::Locale,
    #[serde(
        default,
        rename = "provider",
        deserialize_with = "deserialize_image_providers"
    )]
    pub providers: Vec<ImageProvider>,
}

pub fn create_app() -> Router<()> {
    Router::new()
        .merge(view_routes())
        .nest("/api", api_routes())
        .layer(TraceLayer::new_for_http())
}
