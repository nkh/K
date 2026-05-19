use axum::response::Html;
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "static/admin/"]
struct AdminAssets;

pub async fn admin_page() -> Html<String> {
    match AdminAssets::get("index.html") {
        Some(content) => Html(String::from_utf8_lossy(&content.data).to_string()),
        None => Html("<html><body><h1>vrunner Admin</h1><p>Assets not found.</p></body></html>".to_string()),
    }
}

pub async fn admin_assets() -> Html<String> {
    // TODO: Serve other static assets (CSS, JS) properly
    Html("".to_string())
}
