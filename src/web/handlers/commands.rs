use axum::{
    extract::{Path, State},
    Json,
};
use serde_json::Value;
use std::collections::HashMap;

use crate::web::state::AppState;
use crate::config::merge::merge_command_env;

pub async fn list_commands(
    State(state): State<AppState>,
) -> Json<Value> {
    let commands = state.manager.list();
    let data: Vec<Value> = commands.into_iter()
        .map(|(id, name, pid, certificate)| {
            // Look up exit config for the command
            let exit_info = state.manager.get(&id).map(|h| {
                serde_json::json!({
                    "on_exit": h.exit_config.on_exit.as_deref().unwrap_or(&String::new()),
                    "on_error": h.exit_config.on_error.as_deref().unwrap_or(&String::new()),
                    "exit_timeout": h.exit_config.timeout_secs,
                })
            }).unwrap_or(serde_json::json!(null));
            serde_json::json!({
                "id": id,
                "name": name,
                "pid": pid,
                "status": "running",
                "certificate": certificate,
                "exit": exit_info,
            })
        })
        .collect();
    Json(serde_json::json!({ "status": "ok", "data": data, "error": null }))
}

pub async fn start_command(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let cmd = body.get("cmd").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let args: Vec<String> = body.get("args")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();
    let certificate: Option<String> = body.get("certificate")
        .and_then(|v| v.as_str())
        .map(String::from);

    let on_exit: Option<String> = body.get("on_exit")
        .and_then(|v| v.as_str())
        .map(String::from);

    let on_error: Option<String> = body.get("on_error")
        .and_then(|v| v.as_str())
        .map(String::from);

    let exit_timeout: u64 = body.get("exit_timeout")
        .and_then(|v| v.as_u64())
        .unwrap_or(10);

    // Parse per-command environment variables from the API request
    let command_env: HashMap<String, String> = body.get("env")
        .and_then(|v| v.as_object())
        .map(|obj| {
            obj.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect()
        })
        .unwrap_or_default();

    if cmd.is_empty() {
        return Json(serde_json::json!({
            "status": "error",
            "data": null,
            "error": "Missing 'cmd' field"
        }));
    }

    // Check for no_env flag — when true, skip config-level environment variables
    let no_env: bool = body.get("no_env").and_then(|v| v.as_bool()).unwrap_or(false);

    // Validate certificate name if provided
    if let Some(ref cert_name) = certificate {
        if !state.cert_store.exists(cert_name) {
            return Json(serde_json::json!({
                "status": "error",
                "data": null,
                "error": format!("Certificate '{}' not found in store", cert_name)
            }));
        }
    }

    // Merge per-command env vars on top of config-level env vars (unless no_env)
    let config_env = if no_env {
        crate::config::schema::EnvironmentConfig::default()
    } else {
        state.manager.config().environment.clone()
    };
    let merged_env = merge_command_env(&config_env, command_env);

    match state.manager.spawn_with_exit(cmd, args, certificate, on_exit, on_error, exit_timeout, merged_env).await {
        Ok(id) => Json(serde_json::json!({
            "status": "ok",
            "data": { "id": id },
            "error": null
        })),
        Err(e) => Json(serde_json::json!({
            "status": "error",
            "data": null,
            "error": e.to_string()
        })),
    }
}

pub async fn kill_command(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let signal = body.get("signal").and_then(|v| v.as_str()).map(String::from);
    match state.manager.kill(&id, signal).await {
        Ok(_) => Json(serde_json::json!({
            "status": "ok",
            "data": { "id": id },
            "error": null
        })),
        Err(e) => Json(serde_json::json!({
            "status": "error",
            "data": null,
            "error": e.to_string()
        })),
    }
}

/// Freeze (suspend) a running command via SIGSTOP.
pub async fn freeze_command(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Json<Value> {
    match state.manager.freeze(&id) {
        Ok(_) => Json(serde_json::json!({
            "status": "ok",
            "data": { "id": id, "frozen": true },
            "error": null
        })),
        Err(e) => Json(serde_json::json!({
            "status": "error",
            "data": null,
            "error": e.to_string()
        })),
    }
}

/// Thaw (resume) a frozen command via SIGCONT.
pub async fn thaw_command(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Json<Value> {
    match state.manager.thaw(&id) {
        Ok(_) => Json(serde_json::json!({
            "status": "ok",
            "data": { "id": id, "frozen": false },
            "error": null
        })),
        Err(e) => Json(serde_json::json!({
            "status": "error",
            "data": null,
            "error": e.to_string()
        })),
    }
}

pub async fn shutdown(
    State(state): State<AppState>,
) -> Json<Value> {
    let _ = state.shutdown_tx.send(());
    Json(serde_json::json!({
        "status": "ok",
        "data": { "message": "shutdown initiated" },
        "error": null
    }))
}

pub async fn get_info(
    State(state): State<AppState>,
) -> Json<Value> {
    let commands = state.manager.list();
    let certs = state.cert_store.list();
    let cert_names: Vec<&str> = certs.iter().map(|c| c.name.as_str()).collect();

    Json(serde_json::json!({
        "status": "ok",
        "data": {
            "command_count": commands.len(),
            "certificate_count": certs.len(),
            "certificates": cert_names,
            "auth_enabled": state.auth_token.is_some(),
        },
        "error": null
    }))
}
