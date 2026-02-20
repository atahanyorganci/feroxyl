//! HTTP API routes and app factory.
//!
//! Exposed for integration testing and server setup.

use std::convert::Infallible;

use axum::{
    Json, Router,
    body::Body,
    extract::{Path, Query},
    http::{Response, StatusCode, header},
    response::IntoResponse,
    routing::get,
};
use markup::Render;
use reqwest::{
    Method, Url,
    header::{HeaderName, HeaderValue},
};
use tower_http::trace::TraceLayer;

use crate::engine::{
    ImageProvider, Locale, Provider, RankedImageResult, RankedSearchResult, Safesearch,
    SearchParams, TimeRange, run_meta_image_search, run_meta_search,
};

const DEFAULT_PROVIDERS: &[Provider] = &[
    Provider::DuckDuckGo,
    Provider::Google,
    Provider::Brave,
    Provider::Startpage,
];

const DEFAULT_IMAGE_PROVIDERS: &[ImageProvider] = &[
    ImageProvider::BingImages,
    ImageProvider::GoogleImages,
    ImageProvider::StartpageImages,
    ImageProvider::Unsplash,
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

fn deserialize_image_providers<'de, D>(deserializer: D) -> Result<Vec<ImageProvider>, D::Error>
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
    safesearch: crate::engine::Safesearch,
    #[serde(default)]
    time_range: crate::engine::TimeRange,
    #[serde(default)]
    locale: crate::engine::Locale,
    #[serde(
        default,
        rename = "provider",
        deserialize_with = "deserialize_image_providers"
    )]
    providers: Vec<ImageProvider>,
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

markup::define! {
    SearchBar {
        form[
            "class" = "relative flex items-center w-full border border-stone-200 border-black bg-stone-50 transition-all duration-200 focus-within:border-red-900 focus-within:ring-2 focus-within:ring-red-900/20 focus-within:ring-offset-2 h-14 px-5 rounded",
            "action" = "/search",
            "method" = "GET"
        ] {
            input[
                "name" = "q",
                "placeholder" = "Search the catalogue\u{2026}",
                "class" = "flex-1 bg-transparent text-slate-900 placeholder:text-slate-500 outline-none border-none font-sans ml-3 text-base",
                "spellcheck" = "false",
                "autocomplete" = "off",
                "autofocus" = true
            ] {}
            button[
                "type" = "submit",
                "class" = "shrink-0 rounded-sm bg-red-900 text-white font-sans font-medium transition-all duration-150 hover:bg-red-800 active:scale-95 tracking-wide px-5 py-2 text-sm"
            ] {
                "Search"
            }
        }
    }
}

async fn index() -> impl IntoResponse {
    let providers = ["DuckDuckGo", "Google", "Brave", "Startpage"];
    let template = markup::new! {
        @markup::doctype()
        html {
            head {
                title { "Feroxyl" }
                script[src="https://cdn.jsdelivr.net/npm/@tailwindcss/browser@4"] {}
            }
            body {
                div["class" = "relative flex flex-col min-h-screen bg-stone-50"] {
                    div[
                        "class" = "h-1 w-full",
                        "style" = "background: linear-gradient(90deg, rgb(127 29 29), rgb(217 119 6) 60%, transparent)"
                    ] {}
                    main["class" = "flex-1 flex flex-col items-center justify-center px-6"] {
                        div["class" = "w-full max-w-xl flex flex-col items-center gap-10"] {
                            div["class" = "flex flex-col items-center gap-2 animate-fade-in text-center"] {
                                p["class" = "font-mono text-xs tracking-[0.25em] text-slate-500 uppercase mb-1"] {
                                    "Knowledge Engine"
                                }
                                h1["class" = "font-serif text-5xl font-medium tracking-tight text-slate-900 leading-none"] {
                                    "Feroxyl"
                                }
                                div[
                                    "class" = "w-16 h-px mt-1 bg-amber-500"
                                ] {}
                                p["class" = "text-sm text-slate-500 font-sans mt-1 max-w-xs leading-relaxed"] {
                                    "A private catalogue of the web. Quality sources, combined."
                                }
                            }
                            div [class="w-full"]{
                                @SearchBar {}
                                div[
                                    "class" = "w-full animate-slide-up",
                                    "style" = "animation-delay: 100ms"
                                ] {
                                    p["class" = "text-center text-xs text-slate-500 font-sans mt-3"] {
                                        "Press "
                                        kbd["class" = "px-1.5 py-0.5 rounded-sm bg-stone-100 border border-stone-200 font-mono text-[10px]"] {
                                            "/"
                                        }
                                        " to focus \u{00a0}·\u{00a0} "
                                        kbd["class" = "px-1.5 py-0.5 rounded-sm bg-stone-100 border border-stone-200 font-mono text-[10px]"] {
                                            "↵"
                                        }
                                        " to search"
                                    }
                                }
                            }
                            div[
                                "class" = "flex flex-wrap justify-center gap-2 animate-fade-in",
                                "style" = "animation-delay: 200ms"
                            ] {
                                @for p in providers {
                                    span["class" = "text-xs font-sans px-3 py-1 border border-stone-200 text-slate-500 bg-stone-100"] {
                                        {p}
                                    }
                                }
                            }
                        }
                    }
                    footer["class" = "py-6 border-t border-stone-200 text-center"] {
                        p["class" = "text-xs font-sans text-slate-500 tracking-wide"] {
                            "No tracking \u{00a0}·\u{00a0} No profiling \u{00a0}·\u{00a0} Private by design"
                        }
                    }
                }
            }
        }
    };
    let mut buf = String::new();
    template.render(&mut buf).expect("markup render");
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        Body::from(buf),
    )
}

markup::define! {
    SearchShell<'a>(query: &'a str) {
        html {
            head {
                title { "Feroxyl - " {query} }
            }
            body {
                h1 { "Results for: " {query} }
                div[id = "results"] {
                    p { "Loading..." }
                }
                // This spot is where streamed scripts will arrive
            }
        }
    }

    SearchResultFragment<'a>(results: &'a [RankedSearchResult]) {
        @for result in results.iter() {
            div.result {
                a[href = &result.url] { {&result.title} }
                @if let Some(content) = &result.content {
                    p { {content} }
                }
            }
        }
    }
}

async fn search_handler(Query(SearchQuery { query, .. }): Query<SearchQuery>) -> impl IntoResponse {
    let body = Body::from_stream(async_stream::stream! {
        let shell = SearchShell { query: &query }.to_string();
        yield Ok::<_, Infallible>(shell);

        // 2. Do the actual search (takes time)
        let params = SearchParams {
            query: query,
            safesearch: Safesearch::Off,
            time_range: TimeRange::Any,
            locale: Locale::EnUS,
        };
        let results = run_meta_search(DEFAULT_PROVIDERS, &params).await.unwrap();
        let html = SearchResultFragment { results: &results }.to_string();

        // 3. Send a script that swaps in the results
        let script = format!(
            r#"<script>
                    document.getElementById("results").innerHTML = `{}`;
                </script>"#,
            html.replace('`', r"\`")
        );
        yield Ok::<_, Infallible>(script);
    });

    Response::builder()
        .header("Content-Type", "text/html")
        .header("Transfer-Encoding", "chunked")
        .body(body)
        .unwrap()
}

pub fn create_app() -> Router<()> {
    Router::new()
        .route("/", get(index))
        .route("/search", get(search_handler))
        .route("/api/search", get(search))
        .route("/api/search/image", get(search_image))
        .route("/api/scrape/*path", get(scrape))
        .layer(TraceLayer::new_for_http())
}
