#![cfg(feature = "vrw")]
#![allow(dead_code, unused_imports)]
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::Value;
use std::collections::HashMap;

use crate::web::response::{api_err, api_ok};
use crate::web::state::AppState;

pub async fn get_vtty_full(State(state): State<AppState>, Path(id): Path<String>) -> Json<Value> {
    match state.manager.get(&id) {
        Some(handle) => {
            let ansi = handle.vtty_ansi().await;
            api_ok(serde_json::json!({ "id": id, "content": ansi }))
        }
        None => api_err(format!("Command {} not found", id)),
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
            api_ok(serde_json::json!({
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
            }))
        }
        None => api_err(format!("Command {} not found", id)),
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
            api_ok(serde_json::json!({
                "id": id,
                "screen": label,
                "html": html,
                "alternate_screen": alt_screen,
                "cursor_visible": cursor_visible,
                "dimensions": { "rows": rows, "cols": cols },
            }))
        }
        None => api_err(format!("Command {} not found", id)),
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
            api_ok(serde_json::json!({
                "id": id,
                "offset": offset,
                "limit": limit,
                "content": content
            }))
        }
        None => api_err(format!("Command {} not found", id)),
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
        return api_err("Invalid dimensions: rows must be 1-10000, cols must be 1-1000");
    }

    match state.manager.get(&id) {
        Some(handle) => {
            // Reject resize for exited commands — their terminal rendering is
            // frozen and cannot be meaningfully resized (no live PTY to send
            // SIGWINCH to).  The VTTY buffer is retained as-is for inspection.
            if !handle.is_alive() {
                return api_err("Cannot resize an exited command's terminal");
            }
            // Use resize_pty: resizes PTY master (sends SIGWINCH to child)
            // AND resizes the in-memory VTTY buffer.
            match handle.resize_pty(rows, cols).await {
                Ok(_) => {
                    state
                        .manager
                        .logger()
                        .log("resize", &format!("id={} pid={} name={} rows={} cols={}", id, handle.pid, handle.name, rows, cols));
                    api_ok(serde_json::json!({ "id": id, "rows": rows, "cols": cols }))
                }
                Err(e) => api_err(e.to_string()),
            }
        }
        None => api_err(format!("Command {} not found", id)),
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
        Ok(changed) => api_ok(serde_json::json!({ "id": id, "changed": changed })),
        Err(e) => api_err(e.to_string()),
    }
}

/// GET /api/commands/:id/vtty/text
///
/// Fetch the VTTY buffer as plain text (no ANSI/HTML markup).
pub async fn get_vtty_text(State(state): State<AppState>, Path(id): Path<String>) -> Json<Value> {
    match state.manager.get(&id) {
        Some(handle) => {
            let text = handle.vtty_plain().await;
            api_ok(serde_json::json!({ "id": id, "text": text }))
        }
        None => api_err(format!("Command {} not found", id)),
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
                api_err(e.to_string()),
            )
                .into_response(),
        },
        None => (
            StatusCode::NOT_FOUND,
            api_err(format!("Command {} not found", id)),
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
            api_ok(serde_json::json!({
                "id": id,
                "generation": gen,
                "cursor": { "row": cursor.0, "col": cursor.1 },
                "dimensions": { "rows": rows, "cols": cols },
                "changed_count": diff.changed_count,
                "full_sync_required": diff.changed_count > rows * cols * 9 / 10,
                "cells": diff.cells,
            }))
        }
        None => api_err(format!("Command {} not found", id)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::schema::Config;
    use crate::process::manager::CommandManager;
    use crate::web::certs::CertificateStore;
    use crate::process::handle::CommandHandle;
    use crate::handles::registry::HandleRegistry;
    use crate::vtty::emulator::VttyEmulator;
    use crate::vtty::sink::VttyOutput;
    use std::sync::Arc;

    fn make_app_state() -> AppState {
        let mut config = Config::default();
        config.binary_name = "test".to_string();
        let manager = Arc::new(CommandManager::new(config));
        let (shutdown_tx, _) = tokio::sync::broadcast::channel::<()>(1);
        let cert_store = Arc::new(CertificateStore::new());
        let (vtty_tx, _) = tokio::sync::broadcast::channel::<(String, String)>(16);
        let (log_tx, _) = tokio::sync::broadcast::channel::<String>(16);
        AppState::new(manager, shutdown_tx, None, cert_store, vtty_tx, log_tx)
    }

    fn insert_mock_cmd(mgr: &CommandManager, id: &str, pid: u32) {
        let (stdin_tx, _stdin_rx) = tokio::sync::mpsc::channel::<crate::process::spawner::StdinMessage>(16);
        let (_exit_tx, exit_rx) = tokio::sync::oneshot::channel::<crate::process::spawner::ExitStatus>();
        let (watch_tx, watch_rx) = tokio::sync::watch::channel(false);
        std::mem::forget(watch_tx);
        let emu = VttyEmulator::new(24, 80, 1000);
        let handle = CommandHandle {
            id: id.to_string(), pid,
            name: format!("cmd-{}", id),
            args: vec!["--test".to_string()],
            emulator: std::sync::Arc::new(tokio::sync::RwLock::new(emu)),
            stdin_tx, _exit_rx: exit_rx,
            handle_registry: HandleRegistry::new(),
            certificate: None,
            exit_config: crate::config::schema::ExitConfig::default(),
            spawn_time: std::time::Instant::now(),
            pty_master: None,
            vtty_output: std::sync::Arc::new(VttyOutput::new()),
            exit_rx: watch_rx,
            exit_code: std::sync::Mutex::new(None),
            exit_time: std::sync::Mutex::new(None),
            frozen: std::sync::atomic::AtomicBool::new(false),
            prev_diff_snapshot: tokio::sync::Mutex::new(None),
        };
        mgr.commands_arc().insert(id.to_string(), handle);
    }

    // ─── get_vtty_full ───

    #[tokio::test]
    async fn test_get_vtty_full_missing() {
        let state = make_app_state();
        let result = get_vtty_full(State(state), Path("nope".into())).await;
        assert_eq!(result.0["status"], "error");
        assert!(result.0["error"].as_str().unwrap().contains("not found"));
    }

    #[tokio::test]
    async fn test_get_vtty_full_success() {
        let state = make_app_state();
        insert_mock_cmd(&state.manager, "cmd-1", 1);
        let result = get_vtty_full(State(state), Path("cmd-1".into())).await;
        assert_eq!(result.0["status"], "ok");
        assert_eq!(result.0["data"]["id"], "cmd-1");
        assert!(result.0["data"]["content"].is_string());
    }

    // ─── get_vtty_html ───

    #[tokio::test]
    async fn test_get_vtty_html_missing() {
        let state = make_app_state();
        let params: HashMap<String, String> = HashMap::new();
        let result = get_vtty_html(State(state), Path("nope".into()), Query(params)).await;
        assert_eq!(result.0["status"], "error");
    }

    #[tokio::test]
    async fn test_get_vtty_html_success() {
        let state = make_app_state();
        insert_mock_cmd(&state.manager, "cmd-1", 1);
        let params: HashMap<String, String> = HashMap::new();
        let result = get_vtty_html(State(state), Path("cmd-1".into()), Query(params)).await;
        assert_eq!(result.0["status"], "ok");
        assert_eq!(result.0["data"]["id"], "cmd-1");
        assert!(result.0["data"]["html"].is_string());
        assert!(result.0["data"]["cursor"].is_object());
        assert_eq!(result.0["data"]["dimensions"]["rows"], 24);
        assert_eq!(result.0["data"]["dimensions"]["cols"], 80);
    }

    #[tokio::test]
    async fn test_get_vtty_html_with_scrollback_offset() {
        let state = make_app_state();
        insert_mock_cmd(&state.manager, "cmd-1", 1);
        let mut params: HashMap<String, String> = HashMap::new();
        params.insert("scrollback_offset".to_string(), "5".to_string());
        let result = get_vtty_html(State(state), Path("cmd-1".into()), Query(params)).await;
        assert_eq!(result.0["status"], "ok");
        assert_eq!(result.0["data"]["scrollback_offset"], 5);
    }

    // ─── get_vtty_buffer ───

    #[tokio::test]
    async fn test_get_vtty_buffer_missing() {
        let state = make_app_state();
        let params: HashMap<String, String> = HashMap::new();
        let result = get_vtty_buffer(State(state), Path("nope".into()), Query(params)).await;
        assert_eq!(result.0["status"], "error");
    }

    #[tokio::test]
    async fn test_get_vtty_buffer_current() {
        let state = make_app_state();
        insert_mock_cmd(&state.manager, "cmd-1", 1);
        let params: HashMap<String, String> = HashMap::new();
        let result = get_vtty_buffer(State(state), Path("cmd-1".into()), Query(params)).await;
        assert_eq!(result.0["status"], "ok");
        assert_eq!(result.0["data"]["screen"], "current");
        assert!(result.0["data"]["html"].is_string());
    }

    #[tokio::test]
    async fn test_get_vtty_buffer_main() {
        let state = make_app_state();
        insert_mock_cmd(&state.manager, "cmd-1", 1);
        let mut params: HashMap<String, String> = HashMap::new();
        params.insert("screen".to_string(), "main".to_string());
        let result = get_vtty_buffer(State(state), Path("cmd-1".into()), Query(params)).await;
        assert_eq!(result.0["status"], "ok");
        assert_eq!(result.0["data"]["screen"], "main");
    }

    #[tokio::test]
    async fn test_get_vtty_buffer_alt() {
        let state = make_app_state();
        insert_mock_cmd(&state.manager, "cmd-1", 1);
        let mut params: HashMap<String, String> = HashMap::new();
        params.insert("screen".to_string(), "alt".to_string());
        let result = get_vtty_buffer(State(state), Path("cmd-1".into()), Query(params)).await;
        assert_eq!(result.0["status"], "ok");
        assert_eq!(result.0["data"]["screen"], "alt");
    }

    // ─── get_vtty_partial ───

    #[tokio::test]
    async fn test_get_vtty_partial_missing() {
        let state = make_app_state();
        let params: HashMap<String, String> = HashMap::new();
        let result = get_vtty_partial(State(state), Path("nope".into()), Query(params)).await;
        assert_eq!(result.0["status"], "error");
    }

    #[tokio::test]
    async fn test_get_vtty_partial_success() {
        let state = make_app_state();
        insert_mock_cmd(&state.manager, "cmd-1", 1);
        let params: HashMap<String, String> = HashMap::new();
        let result = get_vtty_partial(State(state), Path("cmd-1".into()), Query(params)).await;
        assert_eq!(result.0["status"], "ok");
        assert_eq!(result.0["data"]["offset"], 0);
        assert_eq!(result.0["data"]["limit"], 50);
        assert!(result.0["data"]["content"].is_string());
    }

    #[tokio::test]
    async fn test_get_vtty_partial_with_params() {
        let state = make_app_state();
        insert_mock_cmd(&state.manager, "cmd-1", 1);
        let mut params: HashMap<String, String> = HashMap::new();
        params.insert("offset".to_string(), "10".to_string());
        params.insert("limit".to_string(), "5".to_string());
        let result = get_vtty_partial(State(state), Path("cmd-1".into()), Query(params)).await;
        assert_eq!(result.0["status"], "ok");
        assert_eq!(result.0["data"]["offset"], 10);
        assert_eq!(result.0["data"]["limit"], 5);
    }

    // ─── resize_vtty ───

    #[tokio::test]
    async fn test_resize_vtty_missing() {
        let state = make_app_state();
        let body = serde_json::json!({"rows": 40, "cols": 120});
        let result = resize_vtty(State(state), Path("nope".into()), Json(body)).await;
        assert_eq!(result.0["status"], "error");
    }

    #[tokio::test]
    async fn test_resize_vtty_invalid_dimensions() {
        let state = make_app_state();
        insert_mock_cmd(&state.manager, "cmd-1", 1);
        let body = serde_json::json!({"rows": 0, "cols": 0});
        let result = resize_vtty(State(state), Path("cmd-1".into()), Json(body)).await;
        assert_eq!(result.0["status"], "error");
        assert!(result.0["error"].as_str().unwrap().contains("Invalid dimensions"));
    }

    #[tokio::test]
    async fn test_resize_vtty_too_large() {
        let state = make_app_state();
        insert_mock_cmd(&state.manager, "cmd-1", 1);
        let body = serde_json::json!({"rows": 20000, "cols": 80});
        let result = resize_vtty(State(state), Path("cmd-1".into()), Json(body)).await;
        assert_eq!(result.0["status"], "error");
    }

    // ─── vtty_changed ───

    #[tokio::test]
    async fn test_vtty_changed_missing() {
        let state = make_app_state();
        let result = vtty_changed(State(state), Path("nope".into())).await;
        assert_eq!(result.0["status"], "error");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_vtty_changed_first_check() {
        let state = make_app_state();
        insert_mock_cmd(&state.manager, "cmd-1", 1);
        let result = vtty_changed(State(state), Path("cmd-1".into())).await;
        assert_eq!(result.0["status"], "ok");
        // First check should return changed=true
        assert_eq!(result.0["data"]["changed"], true);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_vtty_changed_no_change() {
        let state = make_app_state();
        insert_mock_cmd(&state.manager, "cmd-1", 1);
        vtty_changed(State(state.clone()), Path("cmd-1".into())).await;
        let result = vtty_changed(State(state), Path("cmd-1".into())).await;
        assert_eq!(result.0["status"], "ok");
        assert_eq!(result.0["data"]["changed"], false);
    }

    // ─── get_vtty_text ───

    #[tokio::test]
    async fn test_get_vtty_text_missing() {
        let state = make_app_state();
        let result = get_vtty_text(State(state), Path("nope".into())).await;
        assert_eq!(result.0["status"], "error");
    }

    #[tokio::test]
    async fn test_get_vtty_text_success() {
        let state = make_app_state();
        insert_mock_cmd(&state.manager, "cmd-1", 1);
        let result = get_vtty_text(State(state), Path("cmd-1".into())).await;
        assert_eq!(result.0["status"], "ok");
        assert_eq!(result.0["data"]["id"], "cmd-1");
        assert!(result.0["data"]["text"].is_string());
    }

    // ─── get_vtty_diff ───

    #[tokio::test]
    async fn test_get_vtty_diff_missing() {
        let state = make_app_state();
        let result = get_vtty_diff(State(state), Path("nope".into())).await;
        assert_eq!(result.0["status"], "error");
    }

    #[tokio::test]
    async fn test_get_vtty_diff_success() {
        let state = make_app_state();
        insert_mock_cmd(&state.manager, "cmd-1", 1);
        let result = get_vtty_diff(State(state), Path("cmd-1".into())).await;
        assert_eq!(result.0["status"], "ok");
        assert_eq!(result.0["data"]["id"], "cmd-1");
        assert!(result.0["data"]["cells"].is_array());
        assert!(result.0["data"]["dimensions"].is_object());
    }

    // ─── get_vtty_png ───

    #[tokio::test]
    async fn test_get_vtty_png_missing() {
        let state = make_app_state();
        let params: HashMap<String, String> = HashMap::new();
        let result = get_vtty_png(State(state), Path("nope".into()), Query(params)).await;
        assert_eq!(result.status(), StatusCode::NOT_FOUND);
    }
}