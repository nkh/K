use axum::{
    body::Body,
    extract::Path,
    http::{header, StatusCode},
    response::Response,
};
use std::path::PathBuf;

use crate::web::static_assets::AdminAssets;

/// Helper: build a no-cache response for any embedded asset.
fn no_cache_response(mime: &'static str, data: Vec<u8>) -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime)
        .header(header::CACHE_CONTROL, "no-cache, no-store, must-revalidate")
        .header(header::PRAGMA, "no-cache")
        .header(header::EXPIRES, "0")
        .body(Body::from(data))
        .unwrap()
}

pub async fn admin_page() -> Response {
    match AdminAssets::get("index.html") {
        Some(content) => no_cache_response("text/html; charset=utf-8", content.data.to_vec()),
        None => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
            .body(Body::from("<html><body><h1>vrunner Admin</h1><p>Assets not found.</p></body></html>"))
            .unwrap(),
    }
}

/// Serve `/favicon.ico` at the root — browsers request this automatically.
pub async fn admin_favicon() -> Response {
    match AdminAssets::get("favicon.ico") {
        Some(content) => no_cache_response("image/x-icon", content.data.to_vec()),
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("Not found"))
            .unwrap(),
    }
}

pub async fn admin_assets(Path(path): Path<String>) -> Response {
    // Clean the path to prevent directory traversal
    let path = path.trim_start_matches('/');

    // Default to index.html for root
    let asset_path = if path.is_empty() { "index.html" } else { path };

    match AdminAssets::get(asset_path) {
        Some(content) => no_cache_response(guess_mime_type(asset_path), content.data.to_vec()),
        None => {
            Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from("Asset not found"))
                .unwrap()
        }
    }
}

fn guess_mime_type(path: &str) -> &'static str {
    let path_buf = PathBuf::from(path);
    let ext = path_buf
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    match ext {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" => "application/javascript; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        _ => "application/octet-stream",
    }
}
