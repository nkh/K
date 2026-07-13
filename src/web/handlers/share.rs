#![cfg(feature = "vrw")]

use axum::{
    extract::{
        ws::{Message, WebSocket},
        Path, State, WebSocketUpgrade,
    },
    response::{IntoResponse, Response},
    Json,
};
use futures::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::trace;
use crate::trace::{Direction, Source};
use crate::web::response::{api_err, api_ok};
use crate::web::state::AppState;

// ---------------------------------------------------------------------------
// POST /api/commands/:id/share
// ---------------------------------------------------------------------------

/// Create a share token for a command's terminal output.
///
/// Body: { "keyboard": false, "expires_hours": 24, "label": "optional name" }
/// Returns: { "status": "ok", "data": { "token": "...", "url": "...", "expires_at": "..." } }
pub async fn create_share_token(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let keyboard = body
        .get("keyboard")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let expires_hours = body
        .get("expires_hours")
        .and_then(|v| v.as_u64())
        .unwrap_or(24);
    let label = body
        .get("label")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let expires_at = if expires_hours > 0 {
        Some(Instant::now() + Duration::from_secs(expires_hours * 3600))
    } else {
        None
    };

    // Verify the command exists
    let exists = state.manager.get(&id).is_some();
    if !exists {
        return api_err(format!("Command '{}' not found", id));
    }

    let token = uuid::Uuid::new_v4().to_string();
    let share = crate::web::state::ShareToken {
        cmd_id: id.clone(),
        keyboard,
        expires_at,
        label,
    };
    state.share_tokens.insert(token.clone(), share);

    let expires_at_str = expires_at
        .map(|t| {
            let secs = t.duration_since(Instant::now()).as_secs();
            format!("{}h from now", secs / 3600)
        })
        .unwrap_or_else(|| "never".to_string());

    api_ok(serde_json::json!({
        "token": token,
        "url": format!("/share/{}", token),
        "expires_at": expires_at_str,
        "keyboard": keyboard,
    }))
}

// ---------------------------------------------------------------------------
// GET /api/share/:token
// ---------------------------------------------------------------------------

/// Validate a share token and return the command's VTTY HTML.
pub async fn get_share(State(state): State<AppState>, Path(token): Path<String>) -> Json<Value> {
    let Some(entry) = state.share_tokens.get(&token) else {
        return api_err("Invalid or expired share token");
    };

    let share = entry.value().clone();

    // Check expiration
    if let Some(expires) = share.expires_at {
        if Instant::now() >= expires {
            drop(entry);
            state.share_tokens.remove(&token);
            return api_err("Share token has expired");
        }
    }

    let cmd_id = share.cmd_id;
    drop(entry);

    // Fetch the VTTY HTML for the command
    let html = state
        .manager
        .get(&cmd_id)
        .map(|h| async move { h.vtty_html().await })
        .map(|f| tokio::task::block_in_place(|| tokio::runtime::Handle::current().block_on(f)));

    let html_str = html.unwrap_or_default();

    api_ok(serde_json::json!({
        "cmd_id": cmd_id,
        "html": html_str,
        "keyboard": share.keyboard,
    }))
}

// ---------------------------------------------------------------------------
// GET /api/viewer/:id  (auth-protected)
// ---------------------------------------------------------------------------

/// Create a short-lived viewer token for "Open in New Tab" functionality.
///
/// Returns: { "status": "ok", "data": { "cmd_id": "...", "keyboard": true, "token": "..." } }
pub async fn create_viewer_token(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Json<Value> {
    // Verify the command exists
    if state.manager.get(&id).is_none() {
        return api_err(format!("Command '{}' not found", id));
    }

    let token = uuid::Uuid::new_v4().to_string();
    let share = crate::web::state::ShareToken {
        cmd_id: id.clone(),
        keyboard: true,
        expires_at: Some(Instant::now() + Duration::from_secs(3600)), // 1 hour
        label: None,
    };
    state.share_tokens.insert(token.clone(), share);

    api_ok(serde_json::json!({
        "cmd_id": id,
        "keyboard": true,
        "token": token,
    }))
}

// ---------------------------------------------------------------------------
// GET /api/share/:token/ws  (public, share-token authenticated WebSocket)
// ---------------------------------------------------------------------------

/// Validate a share token and return `(cmd_id, keyboard)` if valid, or an error response.
fn validate_share_token(state: &AppState, token: &str) -> Result<(String, bool), Response> {
    let entry = match state.share_tokens.get(token) {
        Some(e) => e,
        None => {
            return Err(
                (
                    axum::http::StatusCode::UNAUTHORIZED,
                    api_err("Invalid or expired share token"),
                )
                    .into_response(),
            );
        }
    };

    let share = entry.value().clone();

    // Check expiration — clean up if expired
    if let Some(expires) = share.expires_at {
        if Instant::now() >= expires {
            drop(entry);
            state.share_tokens.remove(token);
            return Err(
                (
                    axum::http::StatusCode::UNAUTHORIZED,
                    api_err("Share token has expired"),
                )
                    .into_response(),
            );
        }
    }

    let cmd_id = share.cmd_id.clone();
    let keyboard = share.keyboard;
    drop(entry);

    Ok((cmd_id, keyboard))
}

/// WebSocket upgrade endpoint for share-link real-time terminal streaming.
///
/// Auth is via the share token in the URL path (not bearer token).
/// If the share token has `keyboard == false`, incoming `keys`/`paste`/`resize`
/// messages are rejected with an error.
pub async fn ws_share_stream(
    State(state): State<AppState>,
    Path(token): Path<String>,
    ws: WebSocketUpgrade,
) -> Response {
    // Validate the share token
    let (cmd_id, keyboard) = match validate_share_token(&state, &token) {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    // Verify the command exists before upgrading.
    if state.manager.get(&cmd_id).is_none() {
        return (
            axum::http::StatusCode::NOT_FOUND,
            api_err(format!("Command {} not found", cmd_id)),
        )
            .into_response();
    }

    ws.on_upgrade(move |socket| handle_share_vtty_socket(socket, cmd_id, keyboard, state))
}

// ---------------------------------------------------------------------------
// Share VTTY WebSocket handler (mirrors ws.rs handle_vtty_socket, simpler)
// ---------------------------------------------------------------------------

/// Per-session dirty signal throttle (duplicated from ws.rs to avoid refactoring).
struct SessionThrottle {
    window: Duration,
    max_burst: u32,
    sent_in_window: u32,
    window_start: Instant,
    next_allowed: Instant,
}

impl SessionThrottle {
    fn new(max_burst: u32, window_ms: u32) -> Self {
        let window = Duration::from_millis(window_ms as u64);
        let max_burst = if max_burst == 0 { u32::MAX } else { max_burst };
        Self {
            window,
            max_burst,
            sent_in_window: 0,
            window_start: Instant::now(),
            next_allowed: Instant::now(),
        }
    }

    /// Returns true if the dirty event should be forwarded.
    fn should_forward(&mut self, now: Instant) -> bool {
        if self.max_burst >= u32::MAX || self.window.is_zero() {
            return true;
        }

        if now.duration_since(self.window_start) >= self.window {
            self.window_start = now;
            self.sent_in_window = 0;
            self.next_allowed = now;
        }

        if self.sent_in_window >= self.max_burst {
            return false;
        }

        if now < self.next_allowed {
            return false;
        }

        self.sent_in_window += 1;
        self.next_allowed = now + self.window / self.max_burst as u32;
        true
    }
}

/// Generate a short session ID for tracing (duplicated from ws.rs).
fn session_id() -> String {
    let id = uuid::Uuid::new_v4().to_string();
    id[id.len() - 4..].to_string()
}

async fn handle_share_vtty_socket(
    socket: WebSocket,
    cmd_id: String,
    keyboard: bool,
    state: AppState,
) {
    let (mut ws_tx, mut ws_rx) = socket.split();
    let sid = session_id();

    // Send the welcome message.
    let welcome = json!({"type": "connected", "id": &sid, "cmd_id": &cmd_id}).to_string();
    trace::event(
        Direction::Send,
        Source::WebSocket,
        &sid,
        trace::json_msg_type(&welcome),
        &welcome,
        None,
    );
    if ws_tx.send(Message::Text(welcome)).await.is_err() {
        tracing::warn!(?cmd_id, "ws_share: failed to send welcome message");
        return;
    }

    // Per-session throttle for dirty signals.
    let mut throttle = SessionThrottle::new(state.max_burst, state.burst_window_ms);

    // Subscribe to VTTY dirty/close notifications.
    let mut vtty_rx = state.vtty_events.subscribe();

    // Hold a reference to the manager for sending keys and resize.
    let manager = state.manager.clone();
    let watch_id = cmd_id.clone();

    loop {
        tokio::select! {
            // --- Outgoing: VTTY change notifications ---
            result = vtty_rx.recv() => {
                match result {
                    Ok((event_cmd_id, msg_json)) => {
                        if event_cmd_id != watch_id {
                            continue; // Not our command — skip
                        }

                        // vtty_close always passes through unthrottled.
                        let is_close = msg_json.contains("vtty_close");

                        // Throttle vtty_dirty per session.
                        if !is_close && !throttle.should_forward(Instant::now()) {
                            continue;
                        }

                        // Check if command still exists
                        if manager.get(&watch_id).is_none() {
                            let end_msg = json!({"type": "command_ended", "id": &watch_id}).to_string();
                            trace::event(Direction::Send, Source::WebSocket, &sid, "command_ended", &end_msg, None);
                            let _ = ws_tx.send(Message::Text(end_msg)).await;
                            break;
                        }

                        // Forward the dirty/close signal to the client.
                        if ws_tx.send(Message::Text(msg_json)).await.is_err() {
                            tracing::debug!(?watch_id, "ws_share: client disconnected");
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        tracing::debug!(?watch_id, lagged = n, "ws_share: broadcast lagged, will catch up on next dirty");
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        tracing::debug!(?watch_id, "ws_share: broadcast channel closed");
                        break;
                    }
                }
            }

            // --- Incoming: client messages ---
            msg = ws_rx.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        trace::event(Direction::Recv, Source::WebSocket, &sid, trace::json_msg_type(&text), &text, None);
                        handle_share_client_message(&text, &sid, &manager, &watch_id, keyboard, &mut ws_tx).await;
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        tracing::debug!(?watch_id, "ws_share: client closed connection");
                        break;
                    }
                    Some(Ok(Message::Ping(data))) => {
                        let _ = ws_tx.send(Message::Pong(data)).await;
                    }
                    Some(Err(e)) => {
                        tracing::warn!(?watch_id, error = %e, "ws_share: receive error");
                        break;
                    }
                    _ => {} // Ignore binary/pong
                }
            }
        }
    }
}

/// Process a single client message on the share VTTY WebSocket.
///
/// If `keyboard` is false, `keys`/`paste`/`resize` messages are rejected.
async fn handle_share_client_message(
    text: &str,
    sid: &str,
    manager: &Arc<crate::process::manager::CommandManager>,
    id: &String,
    keyboard: bool,
    ws_tx: &mut (impl SinkExt<Message, Error = axum::Error> + Unpin),
) {
    let msg: Value = match serde_json::from_str(text) {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!(error = %e, "ws_share: invalid JSON from client");
            return;
        }
    };

    let msg_type = msg.get("type").and_then(|v| v.as_str()).unwrap_or("");

    match msg_type {
        "keys" => {
            if !keyboard {
                let err_msg = json!({"type": "error", "message": "Keyboard input is disabled for this share link"}).to_string();
                trace::event(Direction::Send, Source::WebSocket, sid, "error", &err_msg, None);
                let _ = ws_tx.send(Message::Text(err_msg)).await;
                return;
            }
            let keys = msg.get("keys").and_then(|v| v.as_str()).unwrap_or("");
            if !keys.is_empty() {
                if let Err(e) = manager.send_keys(id, keys).await {
                    let err_msg = json!({"type": "error", "message": e.to_string()}).to_string();
                    trace::event(Direction::Send, Source::WebSocket, sid, "error", &err_msg, None);
                    let _ = ws_tx.send(Message::Text(err_msg)).await;
                }
            }
        }
        "paste" => {
            if !keyboard {
                let err_msg = json!({"type": "error", "message": "Keyboard input is disabled for this share link"}).to_string();
                trace::event(Direction::Send, Source::WebSocket, sid, "error", &err_msg, None);
                let _ = ws_tx.send(Message::Text(err_msg)).await;
                return;
            }
            let text_val = msg.get("text").and_then(|v| v.as_str()).unwrap_or("");
            if !text_val.is_empty() {
                if let Some(handle) = manager.get(id) {
                    if let Err(e) = handle.send_paste(text_val).await {
                        let err_msg = json!({"type": "error", "message": e.to_string()}).to_string();
                        trace::event(Direction::Send, Source::WebSocket, sid, "error", &err_msg, None);
                        let _ = ws_tx.send(Message::Text(err_msg)).await;
                    }
                }
            }
        }
        "resize" => {
            if !keyboard {
                let err_msg = json!({"type": "error", "message": "Keyboard input is disabled for this share link"}).to_string();
                trace::event(Direction::Send, Source::WebSocket, sid, "error", &err_msg, None);
                let _ = ws_tx.send(Message::Text(err_msg)).await;
                return;
            }
            let rows = msg.get("rows").and_then(|v| v.as_u64()).unwrap_or(24) as u16;
            let cols = msg.get("cols").and_then(|v| v.as_u64()).unwrap_or(80) as u16;
            if let Some(handle) = manager.get(id) {
                if !handle.is_alive() {
                    let err_msg = json!({"type": "error", "message": "Cannot resize an exited command's terminal"}).to_string();
                    trace::event(Direction::Send, Source::WebSocket, sid, "error", &err_msg, None);
                    let _ = ws_tx.send(Message::Text(err_msg)).await;
                } else {
                    match handle.resize_pty(rows, cols).await {
                        Ok(()) => {
                            manager
                                .logger()
                                .log("resize", &format!("id={} pid={} name={} rows={} cols={}", id, handle.pid, handle.name, rows, cols));
                        }
                        Err(e) => {
                            let err_msg = json!({"type": "error", "message": e.to_string()}).to_string();
                            trace::event(Direction::Send, Source::WebSocket, sid, "error", &err_msg, None);
                            let _ = ws_tx.send(Message::Text(err_msg)).await;
                        }
                    }
                }
            }
        }
        "ping" => {
            let pong = json!({"type": "pong"}).to_string();
            trace::event(Direction::Send, Source::WebSocket, sid, "pong", &pong, None);
            let _ = ws_tx.send(Message::Text(pong)).await;
        }
        _ => {
            tracing::warn!(?msg_type, "ws_share: unknown message type");
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::schema::Config;
    use crate::process::manager::CommandManager;
    use crate::process::handle::CommandHandle;
    use crate::handles::registry::HandleRegistry;
    use crate::vtty::emulator::VttyEmulator;
    use crate::vtty::sink::VttyOutput;
    use crate::web::certs::CertificateStore;
    use crate::web::state::AppState;
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
            args: vec![],
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

    #[tokio::test]
    async fn test_create_share_token_success() {
        let state = make_app_state();
        insert_mock_cmd(&state.manager, "cmd-1", 100);
        let body = serde_json::json!({"keyboard": false, "expires_hours": 24});
        let result = create_share_token(State(state.clone()), Path("cmd-1".into()), Json(body)).await;
        assert_eq!(result.0["status"], "ok");
        assert!(result.0["data"]["token"].is_string());
        assert_eq!(result.0["data"]["keyboard"], false);
        assert!(result.0["data"]["url"].as_str().unwrap().starts_with("/share/"));
        assert!(result.0["data"]["expires_at"].as_str().unwrap().contains("from now"));
    }

    #[tokio::test]
    async fn test_create_share_token_command_not_found() {
        let state = make_app_state();
        let body = serde_json::json!({});
        let result = create_share_token(State(state), Path("nonexistent".into()), Json(body)).await;
        assert_eq!(result.0["status"], "error");
        assert!(result.0["error"].as_str().unwrap().contains("not found"));
    }

    #[tokio::test]
    async fn test_create_share_token_never_expires() {
        let state = make_app_state();
        insert_mock_cmd(&state.manager, "cmd-1", 100);
        let body = serde_json::json!({"expires_hours": 0});
        let result = create_share_token(State(state.clone()), Path("cmd-1".into()), Json(body)).await;
        assert_eq!(result.0["status"], "ok");
        assert_eq!(result.0["data"]["expires_at"], "never");
    }

    #[tokio::test]
    async fn test_create_share_token_with_keyboard() {
        let state = make_app_state();
        insert_mock_cmd(&state.manager, "cmd-1", 100);
        let body = serde_json::json!({"keyboard": true});
        let result = create_share_token(State(state), Path("cmd-1".into()), Json(body)).await;
        assert_eq!(result.0["status"], "ok");
        assert_eq!(result.0["data"]["keyboard"], true);
    }

    #[tokio::test]
    async fn test_create_share_token_with_label() {
        let state = make_app_state();
        insert_mock_cmd(&state.manager, "cmd-1", 100);
        let body = serde_json::json!({"keyboard": true, "label": "my terminal"});
        let result = create_share_token(State(state.clone()), Path("cmd-1".into()), Json(body)).await;
        assert_eq!(result.0["status"], "ok");
        let token = result.0["data"]["token"].as_str().unwrap();
        let entry = state.share_tokens.get(token).unwrap();
        assert_eq!(entry.label.as_deref(), Some("my terminal"));
    }

    #[tokio::test]
    async fn test_get_share_invalid_token() {
        let state = make_app_state();
        let result = get_share(State(state), Path("invalid-token".into())).await;
        assert_eq!(result.0["status"], "error");
        assert!(result.0["error"].as_str().unwrap().contains("Invalid or expired"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_get_share_success() {
        let state = make_app_state();
        insert_mock_cmd(&state.manager, "cmd-1", 100);
        // First create a share token
        let body = serde_json::json!({});
        let create_result = create_share_token(State(state.clone()), Path("cmd-1".into()), Json(body)).await;
        let token = create_result.0["data"]["token"].as_str().unwrap().to_string();

        // Now use it
        let result = get_share(State(state), Path(token)).await;
        assert_eq!(result.0["status"], "ok");
        assert_eq!(result.0["data"]["cmd_id"], "cmd-1");
        assert_eq!(result.0["data"]["keyboard"], false);
        // HTML should be a string (may be empty for mock)
        assert!(result.0["data"]["html"].is_string());
    }

    // -----------------------------------------------------------------------
    // New tests for viewer token and WS auth
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_create_viewer_token_success() {
        let state = make_app_state();
        insert_mock_cmd(&state.manager, "cmd-1", 100);

        let result = create_viewer_token(State(state.clone()), Path("cmd-1".into())).await;
        assert_eq!(result.0["status"], "ok");
        assert_eq!(result.0["data"]["cmd_id"], "cmd-1");
        assert_eq!(result.0["data"]["keyboard"], true);
        let token = result.0["data"]["token"].as_str().unwrap();
        assert!(!token.is_empty());

        // Verify the token actually exists in the store
        let entry = state.share_tokens.get(token);
        assert!(entry.is_some());
        let entry = entry.unwrap();
        let share = entry.value();
        assert_eq!(share.cmd_id, "cmd-1");
        assert_eq!(share.keyboard, true);
        assert!(share.expires_at.is_some());
    }

    #[tokio::test]
    async fn test_create_viewer_token_command_not_found() {
        let state = make_app_state();

        let result = create_viewer_token(State(state), Path("nonexistent-cmd".into())).await;
        assert_eq!(result.0["status"], "error");
        assert!(result.0["error"].as_str().unwrap().contains("not found"));
    }

    #[tokio::test]
    async fn test_validate_share_token_for_ws() {
        let state = make_app_state();
        insert_mock_cmd(&state.manager, "cmd-1", 100);

        // Create a share token
        let body = serde_json::json!({"keyboard": false, "expires_hours": 1});
        let create_result = create_share_token(State(state.clone()), Path("cmd-1".into()), Json(body)).await;
        let token = create_result.0["data"]["token"].as_str().unwrap().to_string();

        // Validate via the internal function
        let result = validate_share_token(&state, &token);
        assert!(result.is_ok());
        let (cmd_id, keyboard) = result.unwrap();
        assert_eq!(cmd_id, "cmd-1");
        assert_eq!(keyboard, false);

        // Validate a nonexistent token
        let result = validate_share_token(&state, "bogus-token");
        assert!(result.is_err());

        // Create an already-expired token
        let expired_token = uuid::Uuid::new_v4().to_string();
        state.share_tokens.insert(expired_token.clone(), crate::web::state::ShareToken {
            cmd_id: "cmd-1".to_string(),
            keyboard: true,
            expires_at: Some(Instant::now() - Duration::from_secs(1)),
            label: None,
        });
        let result = validate_share_token(&state, &expired_token);
        assert!(result.is_err());
        // The expired token should have been cleaned up
        assert!(!state.share_tokens.contains_key(&expired_token));
    }
}