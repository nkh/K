use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::Value;
use std::collections::HashMap;

use crate::web::state::AppState;

pub async fn get_vtty_full(State(state): State<AppState>, Path(id): Path<String>) -> Json<Value> {
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
    Query(params): Query<HashMap<String, String>>,
) -> Json<Value> {
    let scrollback_offset = params
        .get("scrollback_offset")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(0);

    match state.manager.get(&id) {
        Some(handle) => {
            let (html, cursor) = if scrollback_offset > 0 {
                // When viewing scrollback, adjust cursor position to be relative
                // to the scrollback viewport (may be out of visible range).
                let (rows, _cols) = handle.dimensions().await;
                let html = handle.vtty_html_scrollback(scrollback_offset, rows).await;
                let (cursor_row, cursor_col) = handle.cursor_position().await;
                // Adjust cursor row relative to scrollback offset
                let total_lines = handle.scrollback_count().await + rows;
                let adj_row = if cursor_row + scrollback_offset + rows >= total_lines {
                    cursor_row + rows // cursor is still in visible area
                } else {
                    rows // cursor is off-screen (above visible area)
                };
                (html, (adj_row, cursor_col))
            } else {
                (handle.vtty_html().await, handle.cursor_position().await)
            };
            let (rows, cols) = handle.dimensions().await;
            let scrollback = handle.scrollback_count().await;
            let alt_screen = handle.is_alternate_screen().await;
            let mouse_tracking = handle.mouse_tracking_enabled().await;
            let mouse_sgr = handle.mouse_sgr_enabled().await;
            let cursor_visible = handle.is_cursor_visible().await;
            let generation = handle.buffer_generation().await;
            Json(serde_json::json!({
                "status": "ok",
                "data": {
                    "id": id,
                    "html": html,
                    "cursor": { "row": cursor.0, "col": cursor.1 },
                    "dimensions": { "rows": rows, "cols": cols },
                    "scrollback_lines": scrollback,
                    "scrollback_offset": scrollback_offset,
                    "alternate_screen": alt_screen,
                    "cursor_visible": cursor_visible,
                    "mouse_tracking": mouse_tracking,
                    "mouse_sgr": mouse_sgr,
                    "generation": generation,
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
    let screen = params
        .get("screen")
        .map(|s| s.as_str())
        .unwrap_or("current");

    match state.manager.get(&id) {
        Some(handle) => {
            let (html, label) = match screen {
                "main" => (handle.vtty_html_main().await, "main"),
                "alt" => (handle.vtty_html_alt().await, "alt"),
                _ => (handle.vtty_html().await, "current"),
            };
            let alt_screen = handle.is_alternate_screen().await;
            let (rows, cols) = handle.dimensions().await;
            let cursor_visible = handle.is_cursor_visible().await;
            Json(serde_json::json!({
                "status": "ok",
                "data": {
                    "id": id,
                    "screen": label,
                    "html": html,
                    "alternate_screen": alt_screen,
                    "cursor_visible": cursor_visible,
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
    let offset = params
        .get("offset")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(0);
    let limit = params
        .get("limit")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(50);

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

    if rows < 1 || cols < 1 || rows > 10000 || cols > 1000 {
        return Json(serde_json::json!({
            "status": "error",
            "data": null,
            "error": "Invalid dimensions: rows must be 1-10000, cols must be 1-1000"
        }));
    }

    match state.manager.get(&id) {
        Some(handle) => {
            // Use resize_pty: resizes PTY master (sends SIGWINCH to child)
            // AND resizes the in-memory VTTY buffer.
            match handle.resize_pty(rows, cols).await {
                Ok(_) => {
                    state
                        .manager
                        .logger()
                        .log("resize", &format!("id={} rows={} cols={}", id, rows, cols));
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
pub async fn vtty_changed(State(state): State<AppState>, Path(id): Path<String>) -> Json<Value> {
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

/// GET /api/commands/:id/vtty/text
///
/// Fetch the VTTY buffer as plain text (no ANSI/HTML markup).
pub async fn get_vtty_text(State(state): State<AppState>, Path(id): Path<String>) -> Json<Value> {
    match state.manager.get(&id) {
        Some(handle) => {
            let text = handle.vtty_plain().await;
            Json(serde_json::json!({
                "status": "ok",
                "data": { "id": id, "text": text },
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

/// GET /api/commands/:id/vtty/png?font_size=14&font_name=
///
/// Render the VTTY buffer as a PNG image using a TrueType/OpenType font
/// and return it as binary data.
/// Query parameters:
/// - `font_size`: pixel height per character cell (default from config, clamped 6–48)
/// - `font_name`: path to a TTF/OTF font file.  When omitted or "monospace", the server
///   searches common system paths for a monospace font.
#[cfg(feature = "png")]
#[cfg_attr(not(feature = "png"), allow(dead_code))]
pub async fn get_vtty_png(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let config = &state.manager.config().vtty;
    let font_size: f32 = params
        .get("font_size")
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(config.screenshot_font_size)
        .clamp(6.0, 48.0);
    let font_path: Option<String> = params
        .get("font_name")
        .cloned()
        .filter(|v| !v.is_empty() && v != "monospace");

    match state.manager.get(&id) {
        Some(handle) => match handle.vtty_png(font_size, font_path.as_deref()).await {
            Ok(png_bytes) => {
                let headers = [
                    ("content-type", "image/png"),
                    (
                        "content-disposition",
                        &format!("attachment; filename=\"{}.png\"", id),
                    ),
                ];
                (StatusCode::OK, headers, png_bytes).into_response()
            }
            Err(e) => (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "status": "error",
                    "data": null,
                    "error": e.to_string()
                })),
            )
                .into_response(),
        },
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "status": "error",
                "data": null,
                "error": format!("Command {} not found", id)
            })),
        )
            .into_response(),
    }
}

/// GET /api/commands/:id/vtty/diff
///
/// Returns a cell-level diff between the last transmitted snapshot and the
/// current buffer state. Used by clients in poll mode (no WebSocket) for
/// incremental DOM patching (Level 3 optimization).
///
/// Response includes `changed_count` and a `cells` array of CellDiff entries.
/// If the terminal dimensions changed since the last diff, returns a flag
/// `full_sync_required: true` and the client should fetch full HTML instead.
pub async fn get_vtty_diff(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Json<Value> {
    match state.manager.get(&id) {
        Some(handle) => {
            let (diff, cursor, dims, gen) = handle.vtty_diff_and_state().await;
            let (rows, cols) = dims;
            Json(serde_json::json!({
                "status": "ok",
                "data": {
                    "id": id,
                    "generation": gen,
                    "cursor": { "row": cursor.0, "col": cursor.1 },
                    "dimensions": { "rows": rows, "cols": cols },
                    "changed_count": diff.changed_count,
                    "full_sync_required": diff.changed_count > rows * cols * 9 / 10,
                    "cells": diff.cells,
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
