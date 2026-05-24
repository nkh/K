use axum::{
    extract::{Path, State, Query},
    Json,
};
use serde_json::Value;
use std::collections::HashMap;

use crate::web::state::AppState;

pub async fn get_vtty_full(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Json<Value> {
    match state.manager.get(&id) {
        Some(handle) => {
            let ansi = handle.vtty_ansi().await;
            Json(serde_json::json!({
                "status": "ok",
                "data": { "id": id, "content": ansi },
                "error": null
            }))
        }
        None => Json(serde_json::json!({
            "status": "error",
            "data": null,
            "error": format!("Command {} not found", id)
        })),
    }
}

pub async fn get_vtty_html(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Json<Value> {
    match state.manager.get(&id) {
        Some(handle) => {
            let html = handle.vtty_html().await;
            let (cursor_row, cursor_col) = handle.cursor_position().await;
            let (rows, cols) = handle.dimensions().await;
            let scrollback = handle.scrollback_count().await;
            let alt_screen = handle.is_alternate_screen().await;
            Json(serde_json::json!({
                "status": "ok",
                "data": {
                    "id": id,
                    "html": html,
                    "cursor": { "row": cursor_row, "col": cursor_col },
                    "dimensions": { "rows": rows, "cols": cols },
                    "scrollback_lines": scrollback,
                    "alternate_screen": alt_screen,
                },
                "error": null
            }))
        }
        None => Json(serde_json::json!({
            "status": "error",
            "data": null,
            "error": format!("Command {} not found", id)
        })),
    }
}

/// GET /api/commands/:id/vtty/buffer?screen=main|alt|current
///
/// Fetch a specific screen buffer as HTML. Supports:
/// - `screen=current` (default): the currently active buffer
/// - `screen=main`: the main buffer (even if alt screen is active)
/// - `screen=alt`: the alternate screen buffer (or last known if switched back)
pub async fn get_vtty_buffer(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Json<Value> {
    let screen = params.get("screen").map(|s| s.as_str()).unwrap_or("current");

    match state.manager.get(&id) {
        Some(handle) => {
            let (html, label) = match screen {
                "main" => (handle.vtty_html_main().await, "main"),
                "alt" => (handle.vtty_html_alt().await, "alt"),
                _ => (handle.vtty_html().await, "current"),
            };
            let alt_screen = handle.is_alternate_screen().await;
            let (rows, cols) = handle.dimensions().await;
            Json(serde_json::json!({
                "status": "ok",
                "data": {
                    "id": id,
                    "screen": label,
                    "html": html,
                    "alternate_screen": alt_screen,
                    "dimensions": { "rows": rows, "cols": cols },
                },
                "error": null
            }))
        }
        None => Json(serde_json::json!({
            "status": "error",
            "data": null,
            "error": format!("Command {} not found", id)
        })),
    }
}

pub async fn get_vtty_partial(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Json<Value> {
    let offset = params.get("offset").and_then(|v| v.parse::<usize>().ok()).unwrap_or(0);
    let limit = params.get("limit").and_then(|v| v.parse::<usize>().ok()).unwrap_or(50);

    match state.manager.get(&id) {
        Some(handle) => {
            let content = handle.vtty_partial(offset, limit).await;
            Json(serde_json::json!({
                "status": "ok",
                "data": {
                    "id": id,
                    "offset": offset,
                    "limit": limit,
                    "content": content
                },
                "error": null
            }))
        }
        None => Json(serde_json::json!({
            "status": "error",
            "data": null,
            "error": format!("Command {} not found", id)
        })),
    }
}

pub async fn resize_vtty(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let rows = body.get("rows").and_then(|v| v.as_u64()).unwrap_or(24) as u16;
    let cols = body.get("cols").and_then(|v| v.as_u64()).unwrap_or(80) as u16;

    if rows < 1 || cols < 1 || rows > 200 || cols > 500 {
        return Json(serde_json::json!({
            "status": "error",
            "data": null,
            "error": "Invalid dimensions: rows must be 1-200, cols must be 1-500"
        }));
    }

    match state.manager.get(&id) {
        Some(handle) => {
            // Use resize_pty: resizes PTY master (sends SIGWINCH to child)
            // AND resizes the in-memory VTTY buffer.
            match handle.resize_pty(rows, cols).await {
                Ok(_) => {
                    state.manager.logger().log("resize", &format!("id={} rows={} cols={}", id, rows, cols));
                    Json(serde_json::json!({
                        "status": "ok",
                        "data": { "id": id, "rows": rows, "cols": cols },
                        "error": null
                    }))
                }
                Err(e) => Json(serde_json::json!({
                    "status": "error",
                    "data": null,
                    "error": e.to_string()
                })),
            }
        }
        None => Json(serde_json::json!({
            "status": "error",
            "data": null,
            "error": format!("Command {} not found", id)
        })),
    }
}

/// GET /api/commands/:id/vtty/changed
///
/// Check whether a command's VTTY buffer has changed since the last poll.
/// This is the lightweight endpoint used by the client in **poll mode**.
/// Returns `{ "changed": true/false }` — no HTML, no diff data.
/// The client should call `GET /api/commands/:id/vtty/html` when this
/// returns `true` to get the updated buffer content.
pub async fn vtty_changed(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Json<Value> {
    match state.manager.has_changed(&id) {
        Ok(changed) => Json(serde_json::json!({
            "status": "ok",
            "data": { "id": id, "changed": changed },
            "error": null
        })),
        Err(e) => Json(serde_json::json!({
            "status": "error",
            "data": null,
            "error": e.to_string()
        })),
    }
}
