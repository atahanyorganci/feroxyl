//! HTML view routes and templates.

use std::convert::Infallible;

use axum::{
    Router,
    body::Body,
    extract::Query,
    http::{Response, StatusCode, header},
    response::IntoResponse,
    routing::get,
};
use markup::Render;

use super::{DEFAULT_PROVIDERS, SearchQuery};
use crate::engine::{
    Locale, RankedSearchResult, Safesearch, SearchParams, TimeRange, run_meta_search,
};

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

async fn search_handler(Query(SearchQuery { query, .. }): Query<SearchQuery>) -> impl IntoResponse {
    let body = Body::from_stream(async_stream::stream! {
        let shell = SearchShell { query: &query }.to_string();
        yield Ok::<_, Infallible>(shell);

        let params = SearchParams {
            query: query.clone(),
            safesearch: Safesearch::Off,
            time_range: TimeRange::Any,
            locale: Locale::EnUS,
        };
        let results = run_meta_search(DEFAULT_PROVIDERS, &params).await.unwrap();
        let html = SearchResultFragment { results: &results }.to_string();

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

pub fn routes() -> Router<()> {
    Router::new()
        .route("/", get(index))
        .route("/search", get(search_handler))
}
