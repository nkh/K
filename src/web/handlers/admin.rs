use axum::{
    body::Body,
    extract::Path,
    http::{header, StatusCode},
    response::{Html, Response},
};
use rust_embed::RustEmbed;
use std::path::PathBuf;

#[derive(RustEmbed)]
#[folder = "static/admin/"]
struct AdminAssets;

pub async fn admin_page() -> Html<String> {
    match AdminAssets::get("index.html") {
        Some(content) => Html(String::from_utf8_lossy(&content.data).to_string()),
        None => Html("<html><body><h1>vrunner Admin</h1><p>Assets not found.</p></body></html>".to_string()),
    }
}

pub async fn admin_assets(Path(path): Path<String>) -> Response {
    // Clean the path to prevent directory traversal
    let path = path.trim_start_matches('/');

    // Default to index.html for root
    let asset_path = if path.is_empty() { "index.html" } else { path };

    match AdminAssets::get(asset_path) {
        Some(content) => {
            let mime_type = guess_mime_type(asset_path);
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime_type)
                .body(Body::from(content.data.to_vec()))
                .unwrap()
        }
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
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "js" => "application/javascript",
        "json" => "application/json",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        _ => "application/octet-stream",
    }
}
