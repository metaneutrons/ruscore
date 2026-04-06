//! Embedded frontend static file serving via rust-embed.

use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use rust_embed::Embed;

#[derive(Embed)]
#[folder = "web/out"]
struct WebAssets;

/// Serve embedded static files. Falls back to index.html for SPA routing.
pub async fn serve_static(uri: axum::http::Uri) -> Response {
    let path = uri.path().trim_start_matches('/');

    // Try exact path first
    if let Some(file) = WebAssets::get(path) {
        return file_response(path, &file.data);
    }

    // Try path + .html (Next.js static export convention)
    let html_path = format!("{path}.html");
    if let Some(file) = WebAssets::get(&html_path) {
        return file_response(&html_path, &file.data);
    }

    // Try path/index.html
    let index_path = if path.is_empty() {
        "index.html".to_string()
    } else {
        format!("{path}/index.html")
    };
    if let Some(file) = WebAssets::get(&index_path) {
        return file_response(&index_path, &file.data);
    }

    // Fall back to 404.html or plain 404
    if let Some(file) = WebAssets::get("404.html") {
        return (
            StatusCode::NOT_FOUND,
            file_headers("404.html"),
            file.data.to_vec(),
        )
            .into_response();
    }

    StatusCode::NOT_FOUND.into_response()
}

fn file_response(path: &str, data: &[u8]) -> Response {
    (StatusCode::OK, file_headers(path), data.to_vec()).into_response()
}

fn file_headers(path: &str) -> [(header::HeaderName, &'static str); 1] {
    let mime = mime_guess::from_path(path)
        .first_raw()
        .unwrap_or("application/octet-stream");
    [(header::CONTENT_TYPE, mime)]
}
