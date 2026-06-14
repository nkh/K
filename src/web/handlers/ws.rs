#![cfg(feature = "vrw")]

use axum::{
    extract::{
        ws::{Message, WebSocket},
        Path, Query, State, WebSocketUpgrade,
    },
    response::{IntoResponse, Response},
};
use futures::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::vtty::emulator::VttyEmulator;
use crate::web::response::api_err;
use crate::web::state::AppState;

// ---------------------------------------------------------------------------
// Helper: build a `vtty_full` JSON message
// ---------------------------------------------------------------------------

/// Build a `vtty_full` JSON message from the given emulator.
///
/// The caller **must** clone the `emulator` `Arc` from the `CommandHandle` and
/// drop any DashMap guard *before* calling this, so we never hold the map lock
/// across the `.await` on the emulator read-lock.
async fn build_vtty_full_json(
    emulator: &Arc<RwLock<VttyEmulator>>,
    cmd_id: &str,
) -> String {
    let (html, cursor_row, cursor_col, rows, cols, alt_screen, cursor_visible, generation) = {
        let emu = emulator.read().await;
        let buf = emu.snapshot();
        let html = crate::vtty::renderer::VttyRenderer::to_html(&buf);
        let (cr, cc) = emu.cursor();
        let (r, c) = emu.dimensions();
        let alt = emu.is_alternate_screen();
        let cv = emu.is_cursor_visible();
        let gen = emu.buffer_generation();
        (html, cr, cc, r, c, alt, cv, gen)
    };
    json!({
        "type": "vtty_full",
        "cmd_id": cmd_id,
        "data": {
            "id": cmd_id,
            "html": html,
            "cursor": {"row": cursor_row, "col": cursor_col},
            "dimensions": {"rows": rows, "cols": cols},
            "alternate_screen": alt_screen,
            "cursor_visible": cursor_visible,
            "generation": generation,
        }
    })
    .to_string()
}

// ---------------------------------------------------------------------------
// VTTY WebSocket stream — GET /api/commands/:id/ws
// ---------------------------------------------------------------------------

/// Upgrade an HTTP request to a WebSocket for real-time VTTY streaming.
///
/// # Security Note
///
/// WebSocket connections cannot set custom HTTP headers from browser JavaScript,
/// so the auth token is accepted as a query parameter (`?token=...`). This is a
/// known limitation of the WebSocket API. The token will appear in:
/// - Server access logs (if request logging is enabled)
/// - Browser history (if the URL was manually entered)
/// - Referrer headers (if the page links elsewhere)
///
/// Mitigations: use short-lived tokens, clear access logs, and prefer TLS
/// to prevent network-level token leakage.
pub async fn ws_vtty_stream(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(params): Query<std::collections::HashMap<String, String>>,
    ws: WebSocketUpgrade,
) -> Response {
    // WebSocket cannot set custom headers, so the client may pass the
    // auth token as a query parameter (?token=...).  Validate it here
    // before allowing the upgrade.
    if let Some(ref expected) = state.auth_token {
        let provided = params.get("token").map(|s| s.as_str());
        match provided {
            Some(t) if t == expected => { /* ok */ }
            _ => {
                return (
                    axum::http::StatusCode::UNAUTHORIZED,
                    api_err("Unauthorized — provide a valid token via ?token=... query parameter"),
                )
                    .into_response();
            }
        }
    }

    // Verify the command exists before upgrading.
    if state.manager.get(&id).is_none() {
        return (
            axum::http::StatusCode::NOT_FOUND,
            api_err(format!("Command {} not found", id)),
        )
            .into_response();
    }

    ws.on_upgrade(move |socket| handle_vtty_socket(socket, id, state))
}

async fn handle_vtty_socket(socket: WebSocket, id: String, state: AppState) {
    let (mut ws_tx, mut ws_rx) = socket.split();

    // Send the welcome message.
    let welcome = json!({"type": "connected", "id": &id, "cmd_id": &id}).to_string();
    if ws_tx.send(Message::Text(welcome)).await.is_err() {
        tracing::warn!(?id, "ws_vtty: failed to send welcome message");
        return;
    }

    // Send the initial full HTML snapshot so the client has a complete picture.
    // Clone the emulator Arc to avoid holding the DashMap lock across .await calls.
    if let Some(handle) = state.manager.get(&id) {
        let emulator = handle.emulator.clone();
        drop(handle); // Release DashMap lock immediately

        let full_msg = build_vtty_full_json(&emulator, &id).await;
        if ws_tx.send(Message::Text(full_msg)).await.is_err() {
            tracing::warn!(?id, "ws_vtty: failed to send initial snapshot");
            return;
        }
    }

    // Subscribe to VTTY change notifications (diff messages).
    let mut vtty_rx = state.vtty_events.subscribe();

    // Subscribe to peer registration events for server-level notifications.
    let mut peer_rx = state.peer_events.subscribe();

    // We need to hold a reference to the manager for sending keys and resize.
    let manager = state.manager.clone();
    let watch_id = id.clone();

    loop {
        tokio::select! {
            // --- Outgoing: VTTY change notifications ---
            result = vtty_rx.recv() => {
                match result {
                    Ok((cmd_id, msg_json)) => {
                        if cmd_id != watch_id {
                            continue; // Not our command — skip
                        }

                        // Check if command still exists
                        if manager.get(&watch_id).is_none() {
                            let end_msg = json!({"type": "command_ended", "id": &watch_id}).to_string();
                            let _ = ws_tx.send(Message::Text(end_msg)).await;
                            break;
                        }

                        // Forward the dirty signal directly to the client.
                        // The message is pre-serialized JSON from the diff watcher.
                        if ws_tx.send(Message::Text(msg_json)).await.is_err() {
                            tracing::debug!(?watch_id, "ws_vtty: client disconnected");
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!(?watch_id, lagged = n, "ws_vtty: broadcast lagged, catching up");
                        // Re-send a full snapshot to resync
                        if let Some(handle) = manager.get(&watch_id) {
                            let emulator = handle.emulator.clone();
                            drop(handle); // Release DashMap lock before .await

                            let resync_msg = build_vtty_full_json(&emulator, &watch_id).await;
                            let _ = ws_tx.send(Message::Text(resync_msg)).await;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        tracing::debug!(?watch_id, "ws_vtty: broadcast channel closed");
                        break;
                    }
                }
            }

            // --- Outgoing: peer registration events ---
            result = peer_rx.recv() => {
                match result {
                    Ok(msg) => {
                        if ws_tx.send(Message::Text(msg)).await.is_err() {
                            tracing::debug!(?watch_id, "ws_vtty: client disconnected (peer event)");
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        // Missed peer events — not critical, client will poll /api/peers
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        // Channel closed during shutdown — ignore
                    }
                }
            }

            // --- Incoming: client messages ---
            msg = ws_rx.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        handle_vtty_client_message(&text, &manager, &watch_id, &mut ws_tx).await;
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        tracing::debug!(?watch_id, "ws_vtty: client closed connection");
                        break;
                    }
                    Some(Ok(Message::Ping(data))) => {
                        let _ = ws_tx.send(Message::Pong(data)).await;
                    }
                    Some(Err(e)) => {
                        tracing::warn!(?watch_id, error = %e, "ws_vtty: receive error");
                        break;
                    }
                    _ => {} // Ignore binary/pong
                }
            }
        }
    }
}

/// Process a single client message on the VTTY WebSocket.
async fn handle_vtty_client_message(
    text: &str,
    manager: &Arc<crate::process::manager::CommandManager>,
    id: &String,
    ws_tx: &mut (impl SinkExt<Message, Error = axum::Error> + Unpin),
) {
    let msg: Value = match serde_json::from_str(text) {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!(error = %e, "ws_vtty: invalid JSON from client");
            return;
        }
    };

    let msg_type = msg.get("type").and_then(|v| v.as_str()).unwrap_or("");

    match msg_type {
        "keys" => {
            let keys = msg.get("keys").and_then(|v| v.as_str()).unwrap_or("");
            if !keys.is_empty() {
                if let Err(e) = manager.send_keys(id, keys).await {
                    let _ = ws_tx
                        .send(Message::Text(
                            json!({"type": "error", "message": e.to_string()}).to_string(),
                        ))
                        .await;
                }
            }
        }
        "resize" => {
            let rows = msg.get("rows").and_then(|v| v.as_u64()).unwrap_or(24) as u16;
            let cols = msg.get("cols").and_then(|v| v.as_u64()).unwrap_or(80) as u16;
            if let Some(handle) = manager.get(id) {
                // Reject resize for exited commands — their terminal rendering
                // is frozen and cannot be meaningfully resized (no live PTY).
                if !handle.is_alive() {
                    let _ = ws_tx
                        .send(Message::Text(
                            json!({"type": "error", "message": "Cannot resize an exited command's terminal"}).to_string(),
                        ))
                        .await;
                } else {
                    match handle.resize_pty(rows, cols).await {
                        Ok(()) => {
                            manager
                                .logger()
                                .log("resize", &format!("id={} pid={} name={} rows={} cols={}", id, handle.pid, handle.name, rows, cols));
                        }
                        Err(e) => {
                            let _ = ws_tx
                                .send(Message::Text(
                                    json!({"type": "error", "message": e.to_string()}).to_string(),
                                ))
                                .await;
                        }
                    }
                }
            }
        }
        "ping" => {
            let _ = ws_tx
                .send(Message::Text(json!({"type": "pong"}).to_string()))
                .await;
        }
        "paste" => {
            let text = msg.get("text").and_then(|v| v.as_str()).unwrap_or("");
            if !text.is_empty() {
                if let Some(handle) = manager.get(id) {
                    if let Err(e) = handle.send_paste(text).await {
                        let _ = ws_tx
                            .send(Message::Text(
                                json!({"type": "error", "message": e.to_string()}).to_string(),
                            ))
                            .await;
                    }
                }
            }
        }
        "request_full" => {
            // Client requests an immediate full HTML resync (e.g., after cell grid desync).
            if let Some(handle) = manager.get(id) {
                let emulator = handle.emulator.clone();
                drop(handle); // Release DashMap lock before .await

                let full_msg = build_vtty_full_json(&emulator, id).await;
                let _ = ws_tx.send(Message::Text(full_msg)).await;
            }
        }
        _ => {
            tracing::warn!(?msg_type, "ws_vtty: unknown message type");
        }
    }
}

// ---------------------------------------------------------------------------
// Log WebSocket stream — GET /api/ws/logs
// ---------------------------------------------------------------------------

/// Upgrade an HTTP request to a WebSocket for real-time log streaming.
pub async fn ws_log_stream(State(state): State<AppState>, ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(move |socket| handle_log_socket(socket, state))
}

async fn handle_log_socket(socket: WebSocket, state: AppState) {
    let (mut ws_tx, mut ws_rx) = socket.split();

    // Send the welcome message.
    let welcome = json!({"type": "connected", "stream": "logs"}).to_string();
    if ws_tx.send(Message::Text(welcome)).await.is_err() {
        tracing::warn!("ws_log: failed to send welcome message");
        return;
    }

    // Subscribe to log events.
    let mut log_rx = state.log_events.subscribe();

    // Subscribe to peer events for server-level notifications.
    let mut peer_rx = state.peer_events.subscribe();

    loop {
        tokio::select! {
            // --- Outgoing: log entries ---
            result = log_rx.recv() => {
                match result {
                    Ok(entry) => {
                        let msg = json!({
                            "type": "log_entry",
                            "data": entry
                        }).to_string();
                        if ws_tx.send(Message::Text(msg)).await.is_err() {
                            tracing::debug!("ws_log: client disconnected");
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!(lagged = n, "ws_log: broadcast lagged, catching up");
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        tracing::debug!("ws_log: broadcast channel closed");
                        break;
                    }
                }
            }

            // --- Outgoing: peer registration events ---
            result = peer_rx.recv() => {
                match result {
                    Ok(msg) => {
                        if ws_tx.send(Message::Text(msg)).await.is_err() {
                            tracing::debug!("ws_log: client disconnected (peer event)");
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {}
                }
            }

            // --- Incoming: client messages ---
            msg = ws_rx.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        // Handle pings.
                        if let Ok(msg) = serde_json::from_str::<Value>(&text) {
                            if msg.get("type").and_then(|v| v.as_str()) == Some("ping") {
                                let _ = ws_tx.send(Message::Text(
                                    json!({"type": "pong"}).to_string()
                                )).await;
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        tracing::debug!("ws_log: client closed connection");
                        break;
                    }
                    Some(Ok(Message::Ping(data))) => {
                        let _ = ws_tx.send(Message::Pong(data)).await;
                    }
                    Some(Err(e)) => {
                        tracing::warn!(error = %e, "ws_log: receive error");
                        break;
                    }
                    _ => {} // Ignore binary/pong
                }
            }
        }
    }
}