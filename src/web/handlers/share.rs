#![cfg(feature = "vrw")]

use axum::{
    extract::{Path, State},
    Json,
};
use serde_json::Value;

use crate::web::response::{api_err, api_ok};
use crate::web::state::AppState;
use std::time::Duration;

/// POST /api/commands/:id/share
/// Create a share token for a command's terminal output.
///
/// Body: { "keyboard": false, "expires_hours": 24 }
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

    let expires_at = if expires_hours > 0 {
        Some(std::time::Instant::now() + Duration::from_secs(expires_hours * 3600))
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
    };
    state.share_tokens.insert(token.clone(), share);

    let expires_at_str = expires_at
        .map(|t| {
            let secs = t.duration_since(std::time::Instant::now()).as_secs();
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

/// GET /api/share/:token
/// Validate a share token and return the command's VTTY HTML.
pub async fn get_share(State(state): State<AppState>, Path(token): Path<String>) -> Json<Value> {
    let Some(entry) = state.share_tokens.get(&token) else {
        return api_err("Invalid or expired share token");
    };

    let share = entry.value().clone();

    // Check expiration
    if let Some(expires) = share.expires_at {
        if std::time::Instant::now() >= expires {
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
}