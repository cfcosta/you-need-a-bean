//! The binary serves the built UI: embedded assets by default, a directory
//! override for development, and an index fallback for non-API paths.

use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use bean_cli::api::{AppState, router};
use bean_cli::ui::UiSource;
use bean_core::loader::load;
use bean_core::model::Ledger;
use tower::util::ServiceExt;

fn app(ui: UiSource) -> Router {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../core/tests/fixtures/model/main.beancount");
    let ledger = Ledger::build(load(&fixture).expect("fixture loads"));
    let state = AppState::new(ledger, 7)
        .with_today((2026, 8, 21))
        .with_ui(ui);
    router(Arc::new(state))
}

async fn get(app: &Router, uri: &str) -> (StatusCode, String, String) {
    let response = app
        .clone()
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .map(|v| v.to_str().unwrap().to_string())
        .unwrap_or_default();
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 22)
        .await
        .unwrap();
    (status, content_type, String::from_utf8_lossy(&bytes).into())
}

#[tokio::test]
async fn serves_the_embedded_ui_at_root() {
    let app = app(UiSource::Embedded);

    let (status, content_type, body) = get(&app, "/").await;
    assert_eq!(status, StatusCode::OK);
    assert!(content_type.starts_with("text/html"), "{content_type}");
    assert!(body.contains("id=\"root\""));

    // The page references its hashed bundle; that asset must be served too.
    let js = body
        .split("src=\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .expect("index.html references a script");
    let (status, content_type, _) = get(&app, js).await;
    assert_eq!(status, StatusCode::OK);
    assert!(content_type.contains("javascript"), "{content_type}");

    let css = body
        .split("href=\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .expect("index.html references a stylesheet");
    let (status, content_type, _) = get(&app, css).await;
    assert_eq!(status, StatusCode::OK);
    assert!(content_type.starts_with("text/css"), "{content_type}");
}

#[tokio::test]
async fn deep_routes_still_load_the_app_bundle() {
    let app = app(UiSource::Embedded);

    // A browser at /some/deep/route resolves the page's asset references
    // against /some/deep/, so the bundle only loads if those references
    // are absolute.
    let (status, _, body) = get(&app, "/some/deep/route").await;
    assert_eq!(status, StatusCode::OK);
    let js = body
        .split("src=\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .expect("index.html references a script");
    assert!(js.starts_with('/'), "asset reference is relative: {js}");

    let (status, content_type, _) = get(&app, js).await;
    assert!(
        status == StatusCode::OK && content_type.contains("javascript"),
        "{js} must serve the bundle, got {status} {content_type}"
    );
}

#[tokio::test]
async fn non_api_paths_fall_back_to_the_index() {
    let app = app(UiSource::Embedded);

    let (status, content_type, body) = get(&app, "/no-such-page").await;
    assert_eq!(status, StatusCode::OK);
    assert!(content_type.starts_with("text/html"), "{content_type}");
    assert!(body.contains("id=\"root\""));

    // API misses keep answering JSON, never HTML.
    let (status, content_type, body) = get(&app, "/api/no-such-endpoint").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(
        content_type.starts_with("application/json"),
        "{content_type}"
    );
    assert!(body.contains("error"));
}

#[tokio::test]
async fn ui_dir_overrides_the_embedded_assets() {
    let root =
        std::env::temp_dir().join(format!("ynab-ui-{}", std::process::id()));
    let dir = root.join("dist");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("index.html"), "<html>custom ui</html>").unwrap();
    std::fs::write(dir.join("app.css"), "body{}").unwrap();
    std::fs::write(root.join("secret.txt"), "top-secret-marker").unwrap();

    let app = app(UiSource::Dir(dir));

    let (status, content_type, body) = get(&app, "/").await;
    assert_eq!(status, StatusCode::OK);
    assert!(content_type.starts_with("text/html"), "{content_type}");
    assert!(body.contains("custom ui"));

    let (status, content_type, _) = get(&app, "/app.css").await;
    assert_eq!(status, StatusCode::OK);
    assert!(content_type.starts_with("text/css"), "{content_type}");

    // Unknown paths serve the directory's index, and traversal never
    // escapes it.
    let (status, _, body) = get(&app, "/missing/page").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("custom ui"));

    let (_, _, body) = get(&app, "/../secret.txt").await;
    assert!(!body.contains("top-secret-marker"));

    std::fs::remove_dir_all(&root).ok();
}
