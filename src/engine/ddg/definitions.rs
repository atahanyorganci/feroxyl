//! `DuckDuckGo` Instant Answer API (definitions)
//!
//! Port of `SearXNG`'s `duckduckgo_definitions.py` engine.
//! Uses the undocumented JSON API at <https://api.duckduckgo.com/>
//!
//! Returns instant answers, definitions, abstracts, and related results.
//! Does not support languages; results are typically in English.

use std::error::Error;

use reqwest::{Method, Url};
use scraper::Html;
use serde::Deserialize;

use crate::engine::{SearchParams, SearchProvider, SearchResult};

const API_URL: &str = "https://api.duckduckgo.com/";

/// Strips HTML tags from a string to plain text.
fn html_to_text(html: &str) -> String {
    let fragment = Html::parse_fragment(html);
    fragment
        .root_element()
        .text()
        .fold(String::new(), |mut acc, s| {
            acc.push_str(s);
            acc
        })
        .trim()
        .to_string()
}

/// Raw result from `DuckDuckGo` Results array.
#[derive(Debug, Deserialize)]
struct DdgResult {
    #[serde(alias = "FirstURL", default)]
    first_url: String,
    #[serde(alias = "Text", default)]
    text: String,
}

/// `DuckDuckGo` Instant Answer API response.
#[derive(Debug, Deserialize)]
struct DdgDefinitionsResponse {
    #[serde(alias = "Answer", default)]
    answer: String,
    #[serde(alias = "AnswerType", default)]
    answer_type: String,
    #[serde(alias = "Definition", default)]
    definition: String,
    #[serde(alias = "DefinitionURL", default)]
    definition_url: String,
    #[serde(alias = "DefinitionSource", default)]
    #[allow(dead_code)]
    definition_source: String,
    #[serde(alias = "Abstract", default)]
    abstract_text: String,
    #[serde(alias = "AbstractURL", default)]
    abstract_url: String,
    #[serde(alias = "AbstractSource", default)]
    #[allow(dead_code)]
    abstract_source: String,
    #[serde(alias = "Heading", default)]
    heading: String,
    #[serde(alias = "Results", default)]
    results: Vec<DdgResult>,
    #[serde(alias = "RelatedTopics", default)]
    related_topics: Vec<serde_json::Value>,
}

/// `DuckDuckGo` Definitions search provider implementing `SearchProvider`.
#[derive(Debug)]
pub struct DuckDuckGoDefinitions {
    results: Vec<SearchResult>,
}

impl Default for DuckDuckGoDefinitions {
    fn default() -> Self {
        Self {
            results: Vec::with_capacity(16),
        }
    }
}

impl DuckDuckGoDefinitions {
    fn parse_related_topics(
        related_topics: &[serde_json::Value],
        heading: &str,
        results: &mut Vec<SearchResult>,
    ) {
        for item in related_topics {
            if let Some(first_url) = item.get("FirstURL").and_then(|v| v.as_str()) {
                let text = item.get("Text").and_then(|v| v.as_str()).unwrap_or("");
                if !is_broken_text(text) {
                    let title = result_to_text(
                        text,
                        item.get("Result").and_then(|v| v.as_str()).unwrap_or(""),
                    );
                    if !title.is_empty() && title != heading {
                        results.push(SearchResult {
                            title: title.clone(),
                            url: first_url.to_string(),
                            content: None,
                        });
                    }
                }
            } else if let Some(topics) = item.get("Topics").and_then(|v| v.as_array()) {
                for topic in topics {
                    let first_url = topic.get("FirstURL").and_then(|v| v.as_str()).unwrap_or("");
                    let text = topic.get("Text").and_then(|v| v.as_str()).unwrap_or("");
                    let result_html = topic.get("Result").and_then(|v| v.as_str()).unwrap_or("");
                    if !is_broken_text(text) {
                        let title = result_to_text(text, result_html);
                        if !title.is_empty() && title != heading && !first_url.is_empty() {
                            results.push(SearchResult {
                                title,
                                url: first_url.to_string(),
                                content: None,
                            });
                        }
                    }
                }
            }
        }
    }
}

/// `DuckDuckGo` may return broken text like ``http://somewhere Related website``.
fn is_broken_text(text: &str) -> bool {
    text.starts_with("http") && text.contains(' ')
}

/// Extract link text from HTML result; fallback to plain text.
fn result_to_text(text: &str, html_result: &str) -> String {
    if html_result.is_empty() {
        return text.to_string();
    }
    let fragment = Html::parse_fragment(html_result);
    let links: Vec<_> = fragment
        .select(&scraper::Selector::parse("a").unwrap())
        .collect();
    let result = if links.is_empty() {
        text.to_string()
    } else {
        links[0].text().fold(String::new(), |mut acc, s| {
            acc.push_str(s);
            acc
        })
    };
    if is_broken_text(&result) {
        String::new()
    } else {
        result.trim().to_string()
    }
}

impl SearchProvider for DuckDuckGoDefinitions {
    fn name() -> &'static str {
        "ddg_definitions"
    }

    fn build_request(
        &mut self,
        params: &SearchParams,
    ) -> Result<reqwest::Request, Box<dyn Error + Send + Sync>> {
        let mut url = Url::parse(API_URL).map_err(|e| std::io::Error::other(e.to_string()))?;
        url.query_pairs_mut()
            .append_pair("q", &params.query)
            .append_pair("format", "json")
            .append_pair("pretty", "0")
            .append_pair("no_redirect", "1")
            .append_pair("d", "1");

        let request = reqwest::Request::new(Method::GET, url);
        Ok(request)
    }

    fn parse_response(&mut self, body: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        let resp: DdgDefinitionsResponse = serde_json::from_str(body).map_err(|e| {
            std::io::Error::other(format!("DuckDuckGo Definitions JSON parse error: {e}"))
        })?;

        let heading = resp.heading.trim();

        // Answer (skip calc, ip types)
        if !resp.answer.is_empty() && resp.answer_type != "calc" && resp.answer_type != "ip" {
            let answer_text = html_to_text(&resp.answer);
            if !answer_text.is_empty() {
                let url = if resp.abstract_url.is_empty() {
                    resp.definition_url.clone()
                } else {
                    resp.abstract_url.clone()
                };
                if url.starts_with("http") {
                    self.results.push(SearchResult {
                        title: if heading.is_empty() {
                            "Answer".to_string()
                        } else {
                            heading.to_string()
                        },
                        url,
                        content: Some(answer_text),
                    });
                }
            }
        }

        // Results (FirstURL, Text)
        for r in &resp.results {
            if !r.first_url.is_empty() && !r.text.is_empty() {
                self.results.push(SearchResult {
                    title: heading.to_string(),
                    url: r.first_url.clone(),
                    content: Some(r.text.clone()),
                });
            }
        }

        // Abstract
        if !resp.abstract_url.is_empty() && !resp.abstract_text.is_empty() {
            let content = html_to_text(&resp.abstract_text);
            if !content.is_empty() {
                self.results.push(SearchResult {
                    title: heading.to_string(),
                    url: resp.abstract_url.clone(),
                    content: Some(content),
                });
            }
        }

        // Definition
        if !resp.definition_url.is_empty() && !resp.definition.is_empty() {
            let content = html_to_text(&resp.definition);
            if !content.is_empty() {
                self.results.push(SearchResult {
                    title: heading.to_string(),
                    url: resp.definition_url.clone(),
                    content: Some(content),
                });
            }
        }

        // Related topics
        Self::parse_related_topics(&resp.related_topics, heading, &mut self.results);

        // If we only have heading + one URL and no content yet, add a simple result
        if !heading.is_empty()
            && self.results.is_empty()
            && (!resp.abstract_url.is_empty() || !resp.definition_url.is_empty())
        {
            let url = if resp.abstract_url.is_empty() {
                resp.definition_url.clone()
            } else {
                resp.abstract_url.clone()
            };
            let mut content = String::new();
            if !resp.definition.is_empty() {
                content.push_str(&html_to_text(&resp.definition));
            }
            if !resp.abstract_text.is_empty() {
                if !content.is_empty() {
                    content.push_str("\n\n");
                }
                content.push_str(&html_to_text(&resp.abstract_text));
            }
            self.results.push(SearchResult {
                title: heading.to_string(),
                url,
                content: if content.is_empty() {
                    None
                } else {
                    Some(content)
                },
            });
        }

        Ok(())
    }

    fn results(&mut self) -> Option<Result<Vec<SearchResult>, Box<dyn Error + Send + Sync>>> {
        if self.results.is_empty() {
            None
        } else {
            Some(Ok(std::mem::take(&mut self.results)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_broken_text() {
        assert!(is_broken_text("http://example.com Related"));
        assert!(is_broken_text("https://foo bar"));
        assert!(!is_broken_text("http://example.com"));
        assert!(!is_broken_text("Normal text"));
    }

    #[test]
    fn test_html_to_text() {
        assert_eq!(html_to_text("Hello <b>world</b>"), "Hello world");
        assert_eq!(html_to_text("<a href=\"/x\">Link</a>"), "Link");
    }
}
