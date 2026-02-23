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
use tokio::time::Instant;

use super::{DEFAULT_PROVIDERS, SearchQuery};
use crate::engine::{
    Locale, RankedSearchResult, Safesearch, SearchParams, TimeRange, run_meta_search,
};

const ACCENT_GRADIENT: &str =
    "background: linear-gradient(90deg, rgb(127 29 29), rgb(217 119 6) 60%, transparent)";
const BODY_CLASS: &str = "min-h-screen bg-stone-50 flex flex-col";
const SLIDE_UP_KEYFRAMES: &str = "@keyframes slideUp{from{opacity:0;transform:translateY(10px)}to{opacity:1;transform:translateY(0)}}";

fn display_url(url: &str) -> &str {
    url.find("://")
        .map(|i| &url[i + 3..])
        .and_then(|after| after.split('/').next())
        .unwrap_or(url)
}

markup::define! {
    /// Outer HTML shell with doctype, html tag, and body with background colors.
    Html<'a, Content: markup::Render>(title: Option<&'a str>, content: Content, extra_styles: Option<&'a str>) {
        @markup::doctype()
        html {
            @Header { title: *title, extra_styles: *extra_styles }
            body["class" = BODY_CLASS] {
                {content}
            }
        }
    }

    /// Header with meta tags, title, fonts, and Tailwind.
    Header<'a>(title: Option<&'a str>, extra_styles: Option<&'a str>) {
        head {
            meta["charset" = "utf-8"] {}
            meta["name" = "viewport", "content" = "width=device-width, initial-scale=1"] {}
            title {
                @if let Some(title) = title {
                    "Feroxyl - " {title}
                }
                else {
                    "Feroxyl"
                }
            }
            link[
                "rel" = "stylesheet",
                "href" = "https://fonts.googleapis.com/css2?family=Literata:ital,opsz,wght@0,7..72,400;0,7..72,500;0,7..72,600&family=Source+Sans+3:wght@400;500;600&family=JetBrains+Mono:wght@400;500&display=swap"
            ] {}
            script["src" = "https://cdn.jsdelivr.net/npm/@tailwindcss/browser@4"] {}
            @if let Some(styles) = extra_styles {
                style { {styles} }
            }
        }
    }

    Footer {
        footer["class" = "border-t border-stone-200 py-4"] {
            p["class" = "max-w-3xl mx-auto px-6 text-xs font-sans text-slate-500"] {
                "Private meta search · No tracking · No profiling"
            }
        }
    }

    AccentBar {
        div[
            "class" = "h-0.5 w-full shrink-0",
            "style" = ACCENT_GRADIENT
        ] {}
    }

    AccentBarThick {
        div[
            "class" = "h-1 w-full",
            "style" = ACCENT_GRADIENT
        ] {}
    }

    SearchBar {
        form[
            "class" = "relative flex items-center w-full border border-stone-200 border-black bg-stone-50 transition-all duration-200 focus-within:border-red-900 focus-within:ring-2 focus-within:ring-red-900/20 focus-within:ring-offset-2 h-14 px-5 rounded",
            "action" = "/search",
            "method" = "GET"
        ] {
            input[
                "name" = "q",
                "placeholder" = "Search the catalogue...",
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

    SearchBarCompact<'a>(query: &'a str) {
        form[
            "class" = "relative flex items-center w-full border border-stone-200 bg-stone-50 transition-all duration-200 focus-within:border-red-900 focus-within:ring-2 focus-within:ring-red-900/20 focus-within:ring-offset-2 h-10 px-4 rounded-sm",
            "action" = "/search",
            "method" = "GET"
        ] {
            input[
                "name" = "q",
                "value" = query,
                "placeholder" = "Search the catalogue...",
                "class" = "flex-1 bg-transparent text-slate-900 placeholder:text-slate-500 outline-none border-none font-sans ml-3 text-sm",
                "spellcheck" = "false",
                "autocomplete" = "off"
            ] {}
            button[
                "type" = "submit",
                "class" = "shrink-0 rounded-sm bg-red-900 text-white font-sans font-medium transition-all duration-150 hover:bg-red-800 active:scale-95 tracking-wide px-3 py-1.5 text-xs"
            ] {
                "Search"
            }
        }
    }

    ResultCard<'a>(result: &'a RankedSearchResult, index: usize) {
        @let hostname = display_url(&result.url);
        @let providers = result.position.iter().map(|(n, _)| *n).collect::<Vec<_>>();
        article[
            "class" = "py-5 border-b border-stone-200 last:border-0",
            "style" = format!("animation: slideUp 0.45s ease {}ms both;", index * 45)
        ] {
            div["class" = "flex items-center gap-2 mb-1.5"] {
                img[
                    "src" = format!("https://www.google.com/s2/favicons?domain={hostname}&sz=16"),
                    "alt" = "",
                    "class" = "w-4 h-4 rounded-sm opacity-70"
                ] {}
                span["class" = "text-xs font-mono text-slate-500 truncate"] {
                    {hostname}
                }
            }
            a[
                "href" = &result.url,
                "target" = "_blank",
                "rel" = "noopener noreferrer",
                "class" = "group flex items-center gap-1.5"
            ] {
                h3["class" = "font-serif text-[1.05rem] font-medium leading-snug text-red-900 group-hover:underline underline-offset-2 decoration-red-900/40 line-clamp-2"] {
                    {&result.title}
                }
                span["class" = "shrink-0 mt-1 text-slate-500 opacity-0 group-hover:opacity-100 transition-opacity pb-0.5"] {
                    "→"
                }
            }
            @if let Some(content) = &result.content {
                p["class" = "mt-2 text-sm text-slate-500 leading-relaxed line-clamp-3 font-sans"] {
                    {content}
                }
            }
            div["class" = "flex flex-wrap gap-1.5 mt-3"] {
                @for p in providers {
                    span["class" = "text-[10px] font-sans font-medium px-2 py-0.5 rounded-sm border border-stone-200 text-slate-500 bg-stone-100"] {
                        {p}
                    }
                }
            }
        }
    }

    SearchResultFragment<'a>(results: &'a [RankedSearchResult], elapsed_str: &'a str, indices: usize) {
        div[
            "class" = "h-[3px] mb-6 rounded-sm bg-[linear-gradient(90deg,rgb(127_29_29),rgb(217_119_6)_60%,transparent)]"
        ] {}
        p["class" = "text-xs font-sans text-slate-500 mb-6"] {
            {elapsed_str}
            span["class" = "text-slate-900 font-medium"] {" " {indices} " indices" }
        }
        div {
            @for (i, result) in results.iter().enumerate() {
                @ResultCard { result, index: i }
            }
        }
    }

    IndexPage<'a>(providers: &'a [&'a str]) {
        div["class" = "relative flex flex-col min-h-screen bg-stone-50"] {
            @AccentBarThick {}
            main["class" = "flex-1 flex flex-col items-center justify-center px-6"] {
                div["class" = "w-full max-w-xl flex flex-col items-center gap-10"] {
                    div["class" = "flex flex-col items-center gap-2 animate-fade-in text-center"] {
                        p["class" = "font-mono text-xs tracking-[0.25em] text-slate-500 uppercase mb-1"] {
                            "Knowledge Engine"
                        }
                        h1["class" = "font-serif text-5xl font-medium tracking-tight text-slate-900 leading-none"] {
                            "Feroxyl"
                        }
                        div["class" = "w-16 h-px mt-1 bg-amber-500"] {}
                        p["class" = "text-sm text-slate-500 font-sans mt-1 max-w-xs leading-relaxed"] {
                            "A private catalogue of the web. Quality sources, combined."
                        }
                    }
                    div["class" = "w-full"] {
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
                                " to focus · "
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
                        @for p in *providers {
                            span["class" = "text-xs font-sans px-3 py-1 border border-stone-200 text-slate-500 bg-stone-100"] {
                                {p}
                            }
                        }
                    }
                }
            }
            @Footer {}
        }
    }

    SearchPageHeader<'a>(query: &'a str) {
        header["class" = "sticky top-0 z-10 bg-stone-50/95 backdrop-blur-sm border-b border-stone-200"] {
            div["class" = "max-w-3xl mx-auto px-6 py-3 flex items-center gap-5"] {
                a[
                    "href" = "/",
                    "class" = "shrink-0 font-serif text-xl font-medium text-slate-900 hover:text-red-900 transition-colors"
                ] {
                    "Feroxyl"
                }
                div["class" = "flex-1"] {
                    @SearchBarCompact { query }
                }
            }
            div["class" = "max-w-3xl mx-auto px-6"] {
                div["class" = "flex -mb-px"] {
                    button["class" = "px-4 py-2 text-sm font-sans font-medium text-red-900 border-b-2 border-red-900"] {
                        "Web"
                    }
                    a[
                        "href" = "#",
                        "class" = "px-4 py-2 text-sm font-sans font-medium text-slate-500 hover:text-slate-900 transition-colors"
                    ] {
                        "Images"
                    }
                    a[
                        "href" = "#",
                        "class" = "px-4 py-2 text-sm font-sans font-medium text-slate-500 hover:text-slate-900 transition-colors"
                    ] {
                        "News"
                    }
                }
            }
        }
    }

    SearchPageLoading {
        div[id = "results"] {
            div["class" = "flex items-center gap-3 text-slate-500 py-16 justify-center"] {
                span["class" = "inline-block w-4 h-4 border-2 border-red-900 border-t-transparent rounded-full animate-spin"] {}
                span["class" = "text-sm font-sans"] { "Consulting sources" }
            }
        }
    }

    SearchShell<'a>(query: &'a str) {
        @Html {
            title: Some(query),
            content: markup::new! {
                @AccentBar {}
                @SearchPageHeader { query }
                main["class" = "flex-1 max-w-3xl w-full mx-auto px-6 py-8"] {
                    @SearchPageLoading {}
                }
                @Footer {}
            },
            extra_styles: Some(SLIDE_UP_KEYFRAMES)
        }
    }
}

async fn index() -> impl IntoResponse {
    let providers = ["DuckDuckGo", "Google", "Brave", "Startpage"];
    let template = markup::new! {
        @Html {
            title: None,
            content: markup::new! {
                @IndexPage { providers: &providers }
            },
            extra_styles: None
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

async fn search(Query(SearchQuery { query, .. }): Query<SearchQuery>) -> impl IntoResponse {
    let body = Body::from_stream(async_stream::stream! {
        let shell = SearchShell { query: &query }.to_string();
        yield Ok::<_, Infallible>(shell);
        let params = SearchParams {
            query,
            safesearch: Safesearch::Off,
            time_range: TimeRange::Any,
            locale: Locale::EnUS,
        };
        let start = Instant::now();
        let results = run_meta_search(DEFAULT_PROVIDERS, &params).await.unwrap();
        let elapsed_str = format!(
            "{} entries retrieved in {:.2}s across",
            results.len(),
            start.elapsed().as_secs_f64()
        );
        let fragment = SearchResultFragment {
            results: &results,
            elapsed_str: &elapsed_str,
            indices: DEFAULT_PROVIDERS.len(),
        };
        let script = format!(
            r#"<script>
                document.getElementById("results").innerHTML = `{}`;
            </script>"#,
            fragment.to_string().replace('`', r"\`")
        );
        yield Ok::<_, Infallible>(script);
    });

    Response::builder()
        .header("Content-Type", "text/html; charset=utf-8")
        .header("Transfer-Encoding", "chunked")
        .body(body)
        .unwrap()
}

pub fn routes() -> Router<()> {
    Router::new()
        .route("/", get(index))
        .route("/search", get(search))
}
