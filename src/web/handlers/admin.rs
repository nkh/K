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
                "<html><body><h1>vrunner Admin</h1><p>Assets not found.</p></body></html>",
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

/// Serve the minimal share page for `/share/{token}`.
/// This is a public, standalone page — no sidebar, no topbar, just the terminal.
pub async fn share_page(State(state): State<AppState>, Path(token): Path<String>) -> Response {
    // Validate token
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

    let keyboard = share.keyboard;
    let cmd_id = share.cmd_id.clone();
    drop(entry);

    let html = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Shared Terminal — vrunner</title>
<link rel="icon" type="image/x-icon" href="/favicon.ico">
<style>
:root {{
    --bg-primary: #0d1117;
    --bg-secondary: #161b22;
    --bg-tertiary: #21262d;
    --border: #30363d;
    --text-primary: #c9d1d9;
    --text-secondary: #8b949e;
    --text-muted: #484f58;
    --accent: #58a6ff;
    --font-mono: 'Cascadia Code', 'Fira Code', 'JetBrains Mono', 'Consolas', monospace;
    --font-size: 10px;
}}
* {{ box-sizing: border-box; margin: 0; padding: 0; }}
body {{ font-family: system-ui, -apple-system, sans-serif; background: var(--bg-primary); color: var(--text-primary); font-size: var(--font-size); overflow: hidden; height: 100vh; }}
#terminal {{ flex: 1; overflow: auto; background: #000; cursor: {keyboard_css}; }}
#terminal pre {{ margin: 0; padding: 0; font-family: var(--font-mono); font-size: var(--font-size); line-height: 1.2; min-width: 100%; min-height: 100%; }}
.badge {{ position: fixed; top: 0.5rem; right: 0.5rem; z-index: 10; background: var(--bg-secondary); border: 1px solid var(--border); border-radius: 4px; padding: 0.2rem 0.5rem; font-size: 0.7rem; color: var(--text-secondary); display: flex; align-items: center; gap: 0.3rem; }}
.badge .dot {{ width: 6px; height: 6px; border-radius: 50%; background: var(--accent); }}
.loading {{ display: flex; align-items: center; justify-content: center; height: 100vh; color: var(--text-muted); font-size: 0.9rem; }}
::-webkit-scrollbar {{ width: 8px; height: 8px; }}
::-webkit-scrollbar-track {{ background: var(--bg-primary); }}
::-webkit-scrollbar-thumb {{ background: var(--border); border-radius: 4px; }}
::-webkit-scrollbar-thumb:hover {{ background: var(--text-muted); }}
</style>
</head>
<body style="display:flex;flex-direction:column;">
<div id="terminal" {tabattr}><pre>Loading terminal...</pre></div>
<div class="badge"><div class="dot"></div> Shared terminal</div>
<script>
const TOKEN = "{token}";
const CMD_ID = "{cmd_id}";
const KEYBOARD = {keyboard_js};
let pollTimer = null;

async function loadTerminal() {{
    try {{
        const res = await fetch('/api/share/' + TOKEN);
        const json = await res.json();
        if (json.status === 'ok' && json.data) {{
            const d = json.data;
            const terminal = document.getElementById('terminal');
            const pre = terminal.querySelector('pre');
            if (pre && d.html !== undefined) {{
                pre.innerHTML = d.html;
            }}
        }} else {{
            const terminal = document.getElementById('terminal');
            const pre = terminal.querySelector('pre');
            if (pre) pre.textContent = 'Error: ' + (json.error || 'Failed to load terminal');
        }}
    }} catch (e) {{
        const terminal = document.getElementById('terminal');
        const pre = terminal.querySelector('pre');
        if (pre) pre.textContent = 'Network error: ' + e.message;
    }}
}}

// Poll every 500ms
function startPoll() {{
    loadTerminal();
    pollTimer = setInterval(loadTerminal, 500);
}}
startPoll();

// Keyboard input if enabled
{keyboard_code}
</script>
</body></html>"#,
        token = token,
        cmd_id = cmd_id,
        keyboard_js = keyboard,
        keyboard_css = if keyboard { "text" } else { "default" },
        tabattr = if keyboard { "tabindex=\"0\"" } else { "" },
        keyboard_code = if keyboard {
            r#"
const terminal = document.getElementById('terminal');
terminal.addEventListener('keydown', async (e) => {
    const keyMap = {
        'Enter': '\r', 'Backspace': '\x7f', 'Tab': '\t', 'Escape': '\x1b',
        'Home': '\x1b[H', 'End': '\x1b[F', 'Delete': '\x1b[3~',
        'ArrowUp': '\x1b[A', 'ArrowDown': '\x1b[B', 'ArrowRight': '\x1b[C', 'ArrowLeft': '\x1b[D',
        'PageUp': '\x1b[5~', 'PageDown': '\x1b[6~', 'Insert': '\x1b[2~',
        'F1': '\x1bOP', 'F2': '\x1bOQ', 'F3': '\x1bOR', 'F4': '\x1bOS',
        'F5': '\x1b[15~', 'F6': '\x1b[17~', 'F7': '\x1b[18~', 'F8': '\x1b[19~',
        'F9': '\x1b[20~', 'F10': '\x1b[21~', 'F11': '\x1b[23~', 'F12': '\x1b[24~',
    };
    let seq = '';
    if (e.ctrlKey && !e.altKey && !e.metaKey) {
        if (e.key.length === 1 && e.key >= 'a' && e.key <= 'z') {
            seq = String.fromCharCode(e.key.charCodeAt(0) - 96);
        }
    } else if (keyMap[e.key]) {
        seq = keyMap[e.key];
    } else if (e.key.length === 1 && !e.ctrlKey && !e.altKey && !e.metaKey) {
        seq = e.key;
    }
    if (!seq) return;
    e.preventDefault();
    try {
        await fetch('/api/commands/' + CMD_ID + '/keys', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ keys: seq }),
        });
        // Trigger a refresh
        setTimeout(loadTerminal, 50);
    } catch (err) {}
});
terminal.focus();
"#
        } else {
            ""
        },
    );

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .header(header::CACHE_CONTROL, "no-cache, no-store, must-revalidate")
        .body(Body::from(html))
        .unwrap()
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
