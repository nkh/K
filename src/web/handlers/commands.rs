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
        .map(|(id, name, args, pid, certificate)| {
            // Look up exit config for the command
            let exit_info = state.manager.get(&id).map(|h| {
                serde_json::json!({
                    "on_exit": h.exit_config.on_exit.as_deref().unwrap_or(&String::new()),
                    "on_error": h.exit_config.on_error.as_deref().unwrap_or(&String::new()),
                    "exit_timeout": h.exit_config.timeout_secs,
                    "retain_on_exit": h.exit_config.retain_on_exit,
                    "snapshot_on_exit": h.exit_config.snapshot_on_exit,
                })
            }).unwrap_or(serde_json::json!(null));
            // Check alive status and compute runtime
            let (alive, runtime_secs, exit_code, exit_time) = state.manager.get(&id).map(|h| {
                let ec = h.exit_code.lock().ok().and_then(|c| *c);
                let et = h.exit_time.lock().ok()
                    .and_then(|guard| *guard)
                    .map(|t| t.elapsed().as_secs() as u64);
                (h.is_alive(), h.runtime_secs(), ec, et)
            }).unwrap_or((false, 0.0, None, None));
            serde_json::json!({
                "id": id,
                "name": name,
                "args": args,
                "pid": pid,
                "alive": alive,
                "runtime_secs": runtime_secs,
                "exit_code": exit_code,
                "exit_time_secs": exit_time,
                "status": if alive { "running" } else { "exited" },
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

    // Per-command VTTY size override (optional)
    let rows: Option<u16> = body.get("rows").and_then(|v| v.as_u64()).map(|v| v as u16);
    let cols: Option<u16> = body.get("cols").and_then(|v| v.as_u64()).map(|v| v as u16);

    // Validate dimensions if provided
    if let (Some(r), Some(c)) = (rows, cols) {
        if r < 1 || c < 1 || r > 200 || c > 500 {
            return Json(serde_json::json!({
                "status": "error",
                "data": null,
                "error": "Invalid dimensions: rows must be 1-200, cols must be 1-500"
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

    let snapshot_on_exit: Option<String> = body.get("snapshot_on_exit")
        .and_then(|v| v.as_str())
        .map(String::from);

    let exit_config = crate::config::schema::ExitConfig {
        on_exit,
        on_error,
        timeout_secs: exit_timeout,
        retain_on_exit: body.get("retain_on_exit").and_then(|v| v.as_bool()).unwrap_or(false),
        snapshot_on_exit,
    };
    match state.manager.spawn(cmd, args, certificate, Some(exit_config), merged_env, rows, cols).await {
        Ok(id) => {
            // Look up the child's OS PID for the response.
            let pid = state.manager.get(&id).map(|h| h.pid).unwrap_or(0);
            Json(serde_json::json!({
                "status": "ok",
                "data": { "id": id, "pid": pid },
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

/// POST /api/commands/kill-pid/:pid
/// Kill a command by its OS PID (as opposed to command UUID).
pub async fn kill_command_by_pid(
    State(state): State<AppState>,
    Path(pid): Path<u32>,
) -> Json<Value> {
    match state.manager.kill_by_pid(pid).await {
        Ok(_) => Json(serde_json::json!({
            "status": "ok",
            "data": { "pid": pid },
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
    let web_config = &state.manager.config().web;

    Json(serde_json::json!({
        "status": "ok",
        "data": {
            "command_count": commands.len(),
            "certificate_count": certs.len(),
            "certificates": cert_names,
            "auth_enabled": state.auth_token.is_some(),
            "web": {
                "update_mode": web_config.update_mode,
                "dirty_check_ms": web_config.dirty_check_ms,
                "default_poll_ms": web_config.default_poll_ms,
            },
        },
        "error": null
    }))
}

/// POST /api/commands/:id/snapshot
/// Store a named snapshot of the command's current VTTY buffer.
/// Body: { "name": "snapshot-name" }
pub async fn snapshot_command(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let name = body.get("name").and_then(|v| v.as_str()).unwrap_or("default").to_string();

    match state.manager.store_snapshot(&id, &name) {
        Ok(meta) => Json(serde_json::json!({
            "status": "ok",
            "data": {
                "id": id,
                "name": meta.name,
                "command_name": meta.command_name,
                "command_args": meta.command_args,
                "pid": meta.pid,
                "timestamp": meta.timestamp.to_rfc3339(),
                "runtime_secs": meta.runtime_secs,
            },
            "error": null
        })),
        Err(e) => Json(serde_json::json!({
            "status": "error",
            "data": null,
            "error": e.to_string()
        })),
    }
}

/// GET /api/commands/:id/snapshots
/// List all snapshots for a command.
pub async fn list_snapshots(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Json<Value> {
    let snapshots = state.manager.list_snapshots(&id);
    Json(serde_json::json!({
        "status": "ok",
        "data": snapshots,
        "error": null
    }))
}

/// POST /api/commands/:id/diff
/// Compute a diff of the current buffer against a stored snapshot.
/// Body: { "name": "snapshot-name" }
pub async fn diff_command(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let name = body.get("name").and_then(|v| v.as_str()).unwrap_or("default").to_string();

    match state.manager.diff_snapshot(&id, &name) {
        Ok(diff) => Json(serde_json::json!({
            "status": "ok",
            "data": {
                "id": id,
                "name": name,
                "width": diff.width,
                "height": diff.height,
                "changed_count": diff.changed_count,
                "cells": diff.cells,
            },
            "error": null
        })),
        Err(e) => Json(serde_json::json!({
            "status": "error",
            "data": null,
            "error": e.to_string()
        })),
    }
}

/// DELETE /api/commands/:id/snapshots/:name
/// Delete a stored snapshot.
pub async fn delete_snapshot(
    State(state): State<AppState>,
    Path((id, name)): Path<(String, String)>,
) -> Json<Value> {
    match state.manager.delete_snapshot(&id, &name) {
        Ok(_) => Json(serde_json::json!({
            "status": "ok",
            "data": { "id": id, "name": name },
            "error": null
        })),
        Err(e) => Json(serde_json::json!({
            "status": "error",
            "data": null,
            "error": e.to_string()
        })),
    }
}

/// GET /api/commands/lookup/:name
/// Find commands by name.  Returns matching commands with alive status
/// and runtime.  Used by the web UI for `/command-name` URL routing.
pub async fn lookup_command(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Json<Value> {
    let commands = state.manager.list();
    let matches: Vec<Value> = commands
        .into_iter()
        .filter(|(_, cmd_name, _, _, _)| {
            // Match the basename of the command (e.g. "/usr/bin/htop" -> "htop")
            cmd_name == &name
                || cmd_name
                    .rsplit('/')
                    .next()
                    .map(|base| base == name)
                    .unwrap_or(false)
        })
        .map(|(id, cmd_name, args, pid, certificate)| {
            let alive = state.manager.get(&id).map(|h| h.is_alive()).unwrap_or(false);
            let runtime = state
                .manager
                .get(&id)
                .map(|h| h.runtime_secs())
                .unwrap_or(0.0);
            serde_json::json!({
                "id": id,
                "name": cmd_name,
                "args": args,
                "pid": pid,
                "alive": alive,
                "runtime_secs": runtime,
                "certificate": certificate,
            })
        })
        .collect();

    Json(serde_json::json!({
        "status": "ok",
        "data": matches,
        "error": null
    }))
}

/// DELETE /api/commands/:id
/// Purge a retained (exited) command from the manager.
/// This permanently discards the VTTY buffer and all associated state.
pub async fn purge_command(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Json<Value> {
    match state.manager.purge(&id) {
        Ok(_) => Json(serde_json::json!({
            "status": "ok",
            "data": { "id": id, "purged": true },
            "error": null
        })),
        Err(e) => Json(serde_json::json!({
            "status": "error",
            "data": null,
            "error": e.to_string()
        })),
    }
}
