#![cfg(feature = "vrw")]

use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, StatusCode, Uri},
    response::Response,
};
use std::path::PathBuf;

use crate::web::state::AppState;
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
            .body(Body::from(
                "<html><body><h1>vrw Admin</h1><p>Assets not found.</p></body></html>",
            ))
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
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("Asset not found"))
            .unwrap(),
    }
}

fn guess_mime_type(path: &str) -> &'static str {
    let path_buf = PathBuf::from(path);
    let ext = path_buf.extension().and_then(|e| e.to_str()).unwrap_or("");

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

/// Serve the share page for `/share/{token}`.
/// Validates the token and serves the viewer.html page with the token embedded.
/// The viewer page handles real-time WebSocket updates.
pub async fn share_page(State(state): State<AppState>, Path(token): Path<String>) -> Response {
    // Validate token exists and is not expired
    let Some(entry) = state.share_tokens.get(&token) else {
        return Response::builder()
            .status(StatusCode::NOT_FOUND)
            .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
            .body(Body::from(r#"<!DOCTYPE html>
<html><head><meta charset="UTF-8"><title>Share Not Found</title></head>
<body style="background:#0d1117;color:#c9d1d9;font-family:system-ui,sans-serif;display:flex;align-items:center;justify-content:center;height:100vh;margin:0;">
<div style="text-align:center;">
<h1 style="color:#f85149;">Shared Terminal Not Found</h1>
<p>This share link is invalid or has expired.</p>
</div>
</body></html>"#))
            .unwrap();
    };

    let share = entry.value().clone();

    // Check expiration
    if let Some(expires) = share.expires_at {
        if std::time::Instant::now() >= expires {
            drop(entry);
            state.share_tokens.remove(&token);
            return Response::builder()
                .status(StatusCode::GONE)
                .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
                .body(Body::from(r#"<!DOCTYPE html>
<html><head><meta charset="UTF-8"><title>Share Expired</title></head>
<body style="background:#0d1117;color:#c9d1d9;font-family:system-ui,sans-serif;display:flex;align-items:center;justify-content:center;height:100vh;margin:0;">
<div style="text-align:center;">
<h1 style="color:#d29922;">Share Link Expired</h1>
<p>This shared terminal has expired.</p>
</div>
</body></html>"#))
                .unwrap();
        }
    }

    // Serve the viewer.html page — the JS will extract the token from the URL path
    // and connect via WebSocket for real-time updates.
    match AdminAssets::get("viewer.html") {
        Some(content) => no_cache_response("text/html; charset=utf-8", content.data.to_vec()),
        None => Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
            .body(Body::from("<h1>Viewer page not found</h1>"))
            .unwrap(),
    }
}

/// Serve the viewer page for `/viewer/{token}` (authenticated "Open in New Tab").
/// Unlike `/share/`, this is for the same user opening a clean terminal view.
/// The token was created via `GET /api/viewer/:cmd_id` (auth-protected).
pub async fn viewer_page(Path(_token): Path<String>) -> Response {
    match AdminAssets::get("viewer.html") {
        Some(content) => no_cache_response("text/html; charset=utf-8", content.data.to_vec()),
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
            .body(Body::from("Viewer page not found"))
            .unwrap(),
    }
}

/// Smart catch-all fallback: if the requested path matches an embedded static
/// asset (e.g. /style.css, /app.js, /favicon-32x32.png), serve it with the
/// correct MIME type.  Otherwise serve index.html — this supports command-name
/// URL routing where /htop, /btop etc. auto-select a command in the JS.
pub async fn smart_fallback(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');

    // Skip empty paths and already-handled prefixes
    if path.is_empty()
        || path.starts_with("api/")
        || path.starts_with("admin")
        || path.starts_with("share/")
        || path.starts_with("viewer/")
    {
        return admin_page().await;
    }

    // Try to serve the embedded asset directly
    if let Some(content) = AdminAssets::get(path) {
        return no_cache_response(guess_mime_type(path), content.data.to_vec());
    }

    // No matching asset — serve index.html for command-name URL routing
    admin_page().await
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── guess_mime_type (data-driven) ───

    #[test]
    fn test_guess_mime_type() {
        let cases: &[(&str, &str)] = &[
            ("index.html", "text/html; charset=utf-8"),
            ("page.htm", "text/html; charset=utf-8"),
            ("style.css", "text/css; charset=utf-8"),
            ("app.js", "application/javascript; charset=utf-8"),
            ("data.json", "application/json; charset=utf-8"),
            ("logo.png", "image/png"),
            ("photo.jpg", "image/jpeg"),
            ("photo.jpeg", "image/jpeg"),
            ("anim.gif", "image/gif"),
            ("icon.svg", "image/svg+xml"),
            ("favicon.ico", "image/x-icon"),
            ("font.woff", "font/woff"),
            ("font.woff2", "font/woff2"),
            ("font.ttf", "font/ttf"),
            ("font.otf", "font/otf"),
            ("file.xyz", "application/octet-stream"),
            ("noextension", "application/octet-stream"),
            ("sub/dir/style.css", "text/css; charset=utf-8"),
        ];
        for (path, expected) in cases {
            assert_eq!(guess_mime_type(path), *expected, "path={}", path);
        }
    }

    // ─── no_cache_response ───

    #[test]
    fn test_no_cache_response_headers() {
        let resp = no_cache_response("text/html; charset=utf-8", b"<h1>Hi</h1>".to_vec());
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            resp.headers().get(header::CONTENT_TYPE).unwrap(),
            "text/html; charset=utf-8"
        );
        assert_eq!(
            resp.headers().get(header::CACHE_CONTROL).unwrap(),
            "no-cache, no-store, must-revalidate"
        );
        assert_eq!(resp.headers().get(header::PRAGMA).unwrap(), "no-cache");
        assert_eq!(resp.headers().get(header::EXPIRES).unwrap(), "0");
    }

    #[test]
    fn test_no_cache_response_binary_data() {
        let data = vec![0x89, 0x50, 0x4E, 0x47]; // PNG magic bytes
        let resp = no_cache_response("image/png", data);
        assert_eq!(resp.status(), StatusCode::OK);
    }
}
