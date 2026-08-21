//! Static serving of the built web UI.
//!
//! The `ui/dist` bundle is compiled into the binary via `rust-embed`, so
//! the executable is self-contained; `--ui-dir` swaps in a directory on
//! disk for iterating on the frontend without rebuilding the binary.

use std::path::{Component, Path, PathBuf};

use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "../../ui/dist"]
struct Assets;

/// Where the UI's files come from.
pub enum UiSource {
    /// The `ui/dist` bundle compiled into the binary.
    Embedded,
    /// A directory on disk (the `--ui-dir` flag).
    Dir(PathBuf),
}

impl UiSource {
    /// The response for a non-API request path, falling back to
    /// `index.html` so the app boots no matter what URL the browser asks
    /// for.
    pub fn respond(&self, path: &str) -> Response {
        if let Some(rel) = sanitize(path)
            && let Some(bytes) = self.read(&rel)
        {
            return file_response(&rel, bytes);
        }
        match self.read("index.html") {
            Some(bytes) => file_response("index.html", bytes),
            None => (
                StatusCode::NOT_FOUND,
                "no UI available: rebuild with ui/dist present or pass --ui-dir",
            )
                .into_response(),
        }
    }

    fn read(&self, rel: &str) -> Option<Vec<u8>> {
        match self {
            UiSource::Embedded => Assets::get(rel).map(|f| f.data.into_owned()),
            UiSource::Dir(dir) => {
                let full = dir.join(rel);
                full.is_file().then(|| std::fs::read(&full).ok()).flatten()
            }
        }
    }
}

/// The request path as a safe relative file path; `None` for anything
/// that would step outside the served directory.
fn sanitize(path: &str) -> Option<String> {
    let trimmed = path.trim_start_matches('/');
    if trimmed.is_empty() {
        return Some("index.html".to_string());
    }
    Path::new(trimmed)
        .components()
        .all(|c| matches!(c, Component::Normal(_)))
        .then(|| trimmed.to_string())
}

fn file_response(name: &str, bytes: Vec<u8>) -> Response {
    let mime = mime_guess::from_path(name).first_or_octet_stream();
    // Bun writes content-hashed chunk names, so those can cache forever;
    // the entry page must revalidate to pick up new hashes.
    let cache = if name.starts_with("chunk-") {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    };
    (
        [
            (header::CONTENT_TYPE, mime.as_ref().to_string()),
            (header::CACHE_CONTROL, cache.to_string()),
        ],
        bytes,
    )
        .into_response()
}
