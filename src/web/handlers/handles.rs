#![cfg(feature = "vrw")]

use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;
use serde_json::Value;

use crate::web::response::{api_err, api_ok};
use crate::web::state::AppState;

/// Request body for attaching a new output handle to a command.
///
/// ```json
/// { "name": "stdout", "sink": "file", "path": "/tmp/output.log" }
/// ```
#[derive(Debug, Deserialize)]
pub struct AddHandleRequest {
    /// Logical name for the handle (must be unique per command).
    pub name: String,
    /// Sink type: "file", "vtty", or "null".
    pub sink: String,
    /// File path for "file" sinks.  Supports `{id}` and `{name}` placeholders.
    /// Ignored for "vtty" and "null" sinks.
    pub path: Option<String>,
}

pub async fn list_handles(State(state): State<AppState>, Path(id): Path<String>) -> Json<Value> {
    match state.manager.get(&id) {
        Some(handle) => {
            let handles = handle.list_handles();
            api_ok(serde_json::json!({ "id": id, "handles": handles }))
        }
        None => api_err(format!("Command {} not found", id)),
    }
}

/// Attach a new output handle to a running command.
///
/// Creates the requested sink (file / vtty / null) and registers it in the
/// command's [`HandleRegistry`](crate::handles::registry::HandleRegistry) so
/// that output is directed to it.
///
/// # Request
///
/// ```json
/// POST /api/commands/:id/handles
/// { "name": "stdout", "sink": "file", "path": "/tmp/output-{id}.log" }
/// ```
pub async fn add_handle(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    // Parse request body
    let req = match serde_json::from_value::<AddHandleRequest>(body) {
        Ok(r) => r,
        Err(e) => {
            return api_err(format!("Invalid request: {}", e));
        }
    };

    state.manager.logger().log(
        "add_handle",
        &format!(
            "id={} name={} sink={} path={:?}",
            id, req.name, req.sink, req.path
        ),
    );

    match state
        .manager
        .register_sink(&id, req.name.clone(), &req.sink, req.path.as_deref())
    {
        Ok(()) => api_ok(serde_json::json!({
            "id": id,
            "name": req.name,
            "sink": req.sink,
            "message": "Handle attached successfully"
        })),
        Err(e) => api_err(e.to_string()),
    }
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
    use serde_json::json;
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

    #[test]
    fn test_add_handle_request_missing_required_fields() {
        let json = r#"{"name":"test"}"#;
        let result = serde_json::from_str::<AddHandleRequest>(json);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_list_handles_not_found() {
        let state = make_app_state();
        let result = list_handles(State(state), Path("nonexistent".into())).await;
        assert_eq!(result.0["status"], "error");
        assert!(result.0["error"].as_str().unwrap().contains("not found"));
    }

    #[tokio::test]
    async fn test_list_handles_empty() {
        let state = make_app_state();
        insert_mock_cmd(&state.manager, "cmd-1", 100);
        let result = list_handles(State(state), Path("cmd-1".into())).await;
        assert_eq!(result.0["status"], "ok");
        assert_eq!(result.0["data"]["id"], "cmd-1");
        assert_eq!(result.0["data"]["handles"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_add_handle_not_found() {
        let state = make_app_state();
        let body = json!({"name": "out", "sink": "null"});
        let result = add_handle(State(state), Path("nonexistent".into()), Json(body)).await;
        assert_eq!(result.0["status"], "error");
        assert!(result.0["error"].as_str().unwrap().contains("not found"));
    }

    #[tokio::test]
    async fn test_add_handle_invalid_body() {
        let state = make_app_state();
        insert_mock_cmd(&state.manager, "cmd-1", 100);
        // Missing "sink" field
        let body = json!({"name": "out"});
        let result = add_handle(State(state), Path("cmd-1".into()), Json(body)).await;
        assert_eq!(result.0["status"], "error");
        assert!(result.0["error"].as_str().unwrap().contains("Invalid request"));
    }

    #[tokio::test]
    async fn test_add_handle_null_sink() {
        let state = make_app_state();
        insert_mock_cmd(&state.manager, "cmd-1", 100);
        let body = json!({"name": "discard", "sink": "null"});
        let result = add_handle(State(state.clone()), Path("cmd-1".into()), Json(body)).await;
        assert_eq!(result.0["status"], "ok");
        assert_eq!(result.0["data"]["id"], "cmd-1");
        assert_eq!(result.0["data"]["name"], "discard");
        assert_eq!(result.0["data"]["sink"], "null");
        // Verify handle is now listed
        let list_result = list_handles(State(state), Path("cmd-1".into())).await;
        assert_eq!(list_result.0["status"], "ok");
        assert!(list_result.0["data"]["handles"].as_array().unwrap().contains(
            &json!("discard")
        ));
    }
}