use axum::{
    extract::{ws::{Message, WebSocket}, Path, State, WebSocketUpgrade},
    response::{IntoResponse, Response},
};
use futures::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::sync::Arc;

use crate::web::state::AppState;

// ---------------------------------------------------------------------------
// VTTY WebSocket stream — GET /api/commands/:id/ws
// ---------------------------------------------------------------------------

/// Upgrade an HTTP request to a WebSocket for real-time VTTY streaming.
///
/// After upgrade, the server:
/// 1. Sends a `{"type":"connected","id":"..."}` welcome message.
/// 2. Subscribes to VTTY change notifications for this command and forwards
///    `{"type":"vtty_update","data":{...}}` messages whenever the terminal
///    content changes.
/// 3. Listens for incoming client messages:
///    - `{"type":"keys","keys":"..."}` — send keystrokes to the command.
///    - `{"type":"resize","rows":N,"cols":N}` — resize the VTTY.
///    - `{"type":"ping"}` — respond with `{"type":"pong"}`.
pub async fn ws_vtty_stream(
    State(state): State<AppState>,
    Path(id): Path<String>,
    ws: WebSocketUpgrade,
) -> Response {
    // Verify the command exists before upgrading.
    if state.manager.get(&id).is_none() {
        return (
            axum::http::StatusCode::NOT_FOUND,
            axum::Json(json!({
                "status": "error",
                "data": null,
                "error": format!("Command {} not found", id)
            })),
        )
            .into_response();
    }

    ws.on_upgrade(move |socket| handle_vtty_socket(socket, id, state))
}

async fn handle_vtty_socket(socket: WebSocket, id: String, state: AppState) {
    let (mut ws_tx, mut ws_rx) = socket.split();

    // Send the welcome message.
    let welcome = json!({"type": "connected", "id": &id}).to_string();
    if ws_tx.send(Message::Text(welcome.into())).await.is_err() {
        tracing::warn!(?id, "ws_vtty: failed to send welcome message");
        return;
    }

    // Subscribe to VTTY change notifications.
    let mut vtty_rx = state.vtty_events.subscribe();

    // We need to hold a reference to the manager for sending keys and resize.
    let manager = state.manager.clone();
    let watch_id = id.clone();

    loop {
        tokio::select! {
            // --- Outgoing: VTTY change notifications ---
            result = vtty_rx.recv() => {
                match result {
                    Ok((cmd_id, html)) => {
                        if cmd_id != watch_id {
                            continue; // Not our command — skip
                        }
                        // Fetch cursor and dimensions from the handle.
                        let msg = if let Some(handle) = manager.get(&watch_id) {
                            let (cursor_row, cursor_col) = handle.cursor_position().await;
                            let (rows, cols) = handle.dimensions().await;
                            let alt_screen = handle.is_alternate_screen().await;
                            json!({
                                "type": "vtty_update",
                                "data": {
                                    "id": &watch_id,
                                    "html": html,
                                    "cursor": {"row": cursor_row, "col": cursor_col},
                                    "dimensions": {"rows": rows, "cols": cols},
                                    "alternate_screen": alt_screen,
                                }
                            }).to_string()
                        } else {
                            // Command was removed while we were subscribed.
                            json!({"type": "command_ended", "id": &watch_id}).to_string()
                        };
                        if ws_tx.send(Message::Text(msg.into())).await.is_err() {
                            tracing::debug!(?watch_id, "ws_vtty: client disconnected");
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!(?watch_id, lagged = n, "ws_vtty: broadcast lagged, catching up");
                        // Continue — we just missed some updates.
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        tracing::debug!(?watch_id, "ws_vtty: broadcast channel closed");
                        break;
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
                    let _ = ws_tx.send(Message::Text(
                        json!({"type": "error", "message": e.to_string()}).to_string().into()
                    )).await;
                }
            }
        }
        "resize" => {
            let rows = msg.get("rows").and_then(|v| v.as_u64()).unwrap_or(24) as u16;
            let cols = msg.get("cols").and_then(|v| v.as_u64()).unwrap_or(80) as u16;
            if let Some(handle) = manager.get(id) {
                match handle.resize(rows, cols).await {
                    Ok(()) => {
                        manager.logger().log("resize", &format!("id={} rows={} cols={}", id, rows, cols));
                    }
                    Err(e) => {
                        let _ = ws_tx.send(Message::Text(
                            json!({"type": "error", "message": e.to_string()}).to_string().into()
                        )).await;
                    }
                }
            }
        }
        "ping" => {
            let _ = ws_tx.send(Message::Text(
                json!({"type": "pong"}).to_string().into()
            )).await;
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
///
/// After upgrade, the server:
/// 1. Sends a `{"type":"connected","stream":"logs"}` welcome message.
/// 2. Subscribes to the command logger's broadcast channel and forwards
///    `{"type":"log_entry","data":"..."}` messages as log entries arrive.
/// 3. Handles pings from the client.
pub async fn ws_log_stream(
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
) -> Response {
    ws.on_upgrade(move |socket| handle_log_socket(socket, state))
}

async fn handle_log_socket(socket: WebSocket, state: AppState) {
    let (mut ws_tx, mut ws_rx) = socket.split();

    // Send the welcome message.
    let welcome = json!({"type": "connected", "stream": "logs"}).to_string();
    if ws_tx.send(Message::Text(welcome.into())).await.is_err() {
        tracing::warn!("ws_log: failed to send welcome message");
        return;
    }

    // Subscribe to log events.
    let mut log_rx = state.log_events.subscribe();

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
                        if ws_tx.send(Message::Text(msg.into())).await.is_err() {
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

            // --- Incoming: client messages ---
            msg = ws_rx.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        // Handle pings.
                        if let Ok(msg) = serde_json::from_str::<Value>(&text) {
                            if msg.get("type").and_then(|v| v.as_str()) == Some("ping") {
                                let _ = ws_tx.send(Message::Text(
                                    json!({"type": "pong"}).to_string().into()
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
