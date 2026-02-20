//! HTTP API integration tests.
//!
//! These tests hit the HTTP endpoints. The image search test requires network access
//! and is ignored by default.
//!
//! Run with: `cargo test --test api -- --ignored`

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use feroxyl::{engine::RankedImageResult, server};
use tower::ServiceExt;

fn assert_valid_image_results(results: &[RankedImageResult]) {
    assert!(!results.is_empty(), "expected at least one image result");
    for r in results {
        assert!(!r.url.is_empty(), "image result url should not be empty");
        assert!(
            r.url.starts_with("http"),
            "image result url should be absolute"
        );
        assert!(
            !r.img_src.is_empty(),
            "image result img_src should not be empty"
        );
        assert!(
            r.img_src.starts_with("http"),
            "image result img_src should be absolute"
        );
    }
}

#[tokio::test]
async fn index_returns_html() {
    let app = server::create_app();

    let request = Request::builder().uri("/").body(Body::empty()).unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok()),
        Some("text/html; charset=utf-8")
    );

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let html = std::str::from_utf8(&body).unwrap();
    assert!(html.contains("<title>Feroxyl</title>"));
    assert!(html.contains("<main"));
}

#[tokio::test]
async fn search_image_endpoint_returns_200_with_query() {
    let app = server::create_app();

    let request = Request::builder()
        .uri("/api/search/image?q=rust")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
#[ignore = "requires network access; run with: cargo test --test api -- --ignored"]
async fn search_image_endpoint_returns_results() {
    let app = server::create_app();

    let request = Request::builder()
        .uri("/api/search/image?q=rust%20logo")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let results: Vec<RankedImageResult> =
        serde_json::from_slice(&body).expect("valid JSON response");

    assert_valid_image_results(&results);
}

#[tokio::test]
#[ignore = "requires network access; run with: cargo test --test api -- --ignored"]
async fn search_image_google_images_returns_results() {
    let app = server::create_app();

    let request = Request::builder()
        .uri("/api/search/image?q=rust%20logo&provider=google_images")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let results: Vec<RankedImageResult> =
        serde_json::from_slice(&body).expect("valid JSON response");

    assert_valid_image_results(&results);
}
