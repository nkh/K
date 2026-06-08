#![cfg(feature = "vrw")]
#![allow(dead_code, unused_imports)]
use axum::{
    extract::{Path, State},
    Json,
};
use serde_json::Value;
use std::collections::HashMap;

use crate::config::merge::merge_command_env;
use crate::web::handlers::resources::read_proc_stats;
use crate::web::state::AppState;

pub async fn list_commands(State(state): State<AppState>) -> Json<Value> {
    let commands = state.manager.list();
    let data: Vec<Value> = commands
        .into_iter()
        .enumerate()
        .map(|(idx, (id, name, args, pid, certificate))| {
            // Look up exit config for the command
            let exit_info = state
                .manager
                .get(&id)
                .map(|h| {
                    serde_json::json!({
                        "on_exit": h.exit_config.on_exit.as_deref().unwrap_or(""),
                        "on_error": h.exit_config.on_error.as_deref().unwrap_or(""),
                        "exit_timeout": h.exit_config.timeout_secs,
                        "retain_on_exit": h.exit_config.retain_on_exit,
                        "snapshot_on_exit": h.exit_config.snapshot_on_exit,
                    })
                })
                .unwrap_or(serde_json::json!(null));
            // Check alive status and compute runtime
            let (alive, runtime_secs, exit_code, exit_time, frozen) = state
                .manager
                .get(&id)
                .map(|h| {
                    let ec = h.exit_code.lock().ok().and_then(|c| *c);
                    let et = h
                        .exit_time
                        .lock()
                        .ok()
                        .and_then(|guard| *guard)
                        .map(|t| t.elapsed().as_secs());
                    (h.is_alive(), h.runtime_secs(), ec, et, h.is_frozen())
                })
                .unwrap_or((false, 0.0, None, None, false));
            serde_json::json!({
                "id": id,
                "name": name,
                "args": args,
                "pid": pid,
                "alive": alive,
                "frozen": frozen,
                "runtime_secs": runtime_secs,
                "exit_code": exit_code,
                "exit_time_secs": exit_time,
                "status": if frozen { "frozen" } else if alive { "running" } else { "exited" },
                "certificate": certificate,
                "exit": exit_info,
                "spawn_order": idx,
            })
        })
        .collect();
    Json(serde_json::json!({ "status": "ok", "data": data, "error": null }))
}

pub async fn start_command(State(state): State<AppState>, Json(body): Json<Value>) -> Json<Value> {
    let cmd = body
        .get("cmd")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let args: Vec<String> = body
        .get("args")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let certificate: Option<String> = body
        .get("certificate")
        .and_then(|v| v.as_str())
        .map(String::from);

    let on_exit: Option<String> = body
        .get("on_exit")
        .and_then(|v| v.as_str())
        .map(String::from);

    let on_error: Option<String> = body
        .get("on_error")
        .and_then(|v| v.as_str())
        .map(String::from);

    let exit_timeout: u64 = body
        .get("exit_timeout")
        .and_then(|v| v.as_u64())
        .unwrap_or(10);

    // Parse per-command environment variables from the API request
    let command_env: HashMap<String, String> = body
        .get("env")
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
    let no_env: bool = body
        .get("no_env")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

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

    // Working directory override (optional)
    let dir: Option<String> = body.get("dir").and_then(|v| v.as_str()).map(String::from);

    // Validate dimensions if provided
    if let (Some(r), Some(c)) = (rows, cols) {
        if r < 1 || c < 1 || r > 10000 || c > 1000 {
            return Json(serde_json::json!({
                "status": "error",
                "data": null,
                "error": "Invalid dimensions: rows must be 1-10000, cols must be 1-1000"
            }));
        }
    }

    // Validate working directory if provided
    if let Some(ref d) = dir {
        let path = std::path::Path::new(d);
        if !path.is_dir() {
            return Json(serde_json::json!({
                "status": "error",
                "data": null,
                "error": format!("Working directory '{}' does not exist or is not a directory", d)
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

    let snapshot_on_exit: Option<String> = body
        .get("snapshot_on_exit")
        .and_then(|v| v.as_str())
        .map(String::from);

    let exit_config = crate::config::schema::ExitConfig {
        on_exit,
        on_error,
        timeout_secs: exit_timeout,
        retain_on_exit: body
            .get("retain_on_exit")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        snapshot_on_exit,
    };
    match state
        .manager
        .spawn(
            cmd,
            args,
            certificate,
            Some(exit_config),
            merged_env,
            rows,
            cols,
            dir,
        )
        .await
    {
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
    let signal = body
        .get("signal")
        .and_then(|v| v.as_str())
        .map(String::from);
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
pub async fn freeze_command(State(state): State<AppState>, Path(id): Path<String>) -> Json<Value> {
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
pub async fn thaw_command(State(state): State<AppState>, Path(id): Path<String>) -> Json<Value> {
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

/// POST /api/commands/:id/restart
/// Atomically restart a command: spawn a new instance FIRST, then kill the
/// old one.  This ensures the command list is never empty during the
/// restart, preventing the display loop or headless mode from triggering
/// an unwanted server shutdown.
pub async fn restart_command(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    // Look up the existing command to get its name and args
    let (cmd_name, cmd_args) = match state.manager.get(&id) {
        Some(h) => (h.name.clone(), h.args.clone()),
        None => {
            return Json(serde_json::json!({
                "status": "error",
                "data": null,
                "error": format!("Command '{}' not found", id)
            }));
        }
    };

    // Allow the request body to override cmd/args (for flexibility).
    // If not provided, reuse the old command's values.
    let override_cmd = body.get("cmd").and_then(|v| v.as_str()).map(String::from);
    let override_args: Option<Vec<String>> = body
        .get("args")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        });

    let final_cmd = override_cmd.unwrap_or(cmd_name);
    let final_args = override_args.unwrap_or(cmd_args);

    // Step 1: Spawn the new command BEFORE killing the old one.
    // This keeps the command list non-empty so the server doesn't
    // interpret the kill as "all commands exited → shutdown".
    match state
        .manager
        .spawn(
            final_cmd,
            final_args,
            None,
            None,
            std::collections::HashMap::new(),
            None,
            None,
            None,
        )
        .await
    {
        Ok(new_id) => {
            let new_pid = state.manager.get(&new_id).map(|h| h.pid).unwrap_or(0);
            // Step 2: Kill the old command now that the replacement is running.
            let _ = state.manager.kill(&id, None).await;
            tracing::info!(
                old_id = %id,
                new_id = %new_id,
                "Restarted command"
            );
            Json(serde_json::json!({
                "status": "ok",
                "data": { "id": new_id, "pid": new_pid },
                "error": null
            }))
        }
        Err(e) => Json(serde_json::json!({
            "status": "error",
            "data": null,
            "error": format!("Failed to spawn replacement command: {}", e)
        })),
    }
}

pub async fn shutdown(State(state): State<AppState>) -> Json<Value> {
    let _ = state.shutdown_tx.send(());
    Json(serde_json::json!({
        "status": "ok",
        "data": { "message": "shutdown initiated" },
        "error": null
    }))
}

pub async fn get_info(State(state): State<AppState>) -> Json<Value> {
    let commands = state.manager.list();
    let certs = state.cert_store.list();
    let cert_names: Vec<&str> = certs.iter().map(|c| c.name.as_str()).collect();
    let web_config = &state.manager.config().web;
    let vtty_config = &state.manager.config().vtty;
    let server_config = &state.manager.config().server;

    Json(serde_json::json!({
        "status": "ok",
        "data": {
            "command_count": commands.len(),
            "certificate_count": certs.len(),
            "certificates": cert_names,
            "auth_enabled": state.auth_token.is_some(),
            "server_name": server_config.name,
            "web": {
                "update_mode": web_config.update_mode,
                "dirty_check_ms": web_config.dirty_check_ms,
                "default_poll_ms": web_config.default_poll_ms,
            },
            "vtty": {
                "screenshot_font_size": vtty_config.screenshot_font_size,
                "screenshot_font_name": vtty_config.screenshot_font_name,
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
    let name = body
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("default")
        .to_string();

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
pub async fn list_snapshots(State(state): State<AppState>, Path(id): Path<String>) -> Json<Value> {
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
    let name = body
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("default")
        .to_string();

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
            let (alive, frozen, runtime) = state
                .manager
                .get(&id)
                .map(|h| (h.is_alive(), h.is_frozen(), h.runtime_secs()))
                .unwrap_or((false, false, 0.0));
            serde_json::json!({
                "id": id,
                "name": cmd_name,
                "args": args,
                "pid": pid,
                "alive": alive,
                "frozen": frozen,
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

/// POST /api/commands/:id/keep
/// Tag a running command so its terminal rendering is kept after exit.
/// Sets `retain_on_exit = true` on the command's exit configuration.
pub async fn keep_command(State(state): State<AppState>, Path(id): Path<String>) -> Json<Value> {
    match state.manager.keep(&id) {
        Ok(_) => Json(serde_json::json!({
            "status": "ok",
            "data": { "id": id, "retain_on_exit": true },
            "error": null
        })),
        Err(e) => Json(serde_json::json!({
            "status": "error",
            "data": null,
            "error": e.to_string()
        })),
    }
}

/// POST /api/commands/:id/unkeep
/// Remove the keep tag from a command. The command will be removed from
/// the manager when it exits.
pub async fn unkeep_command(State(state): State<AppState>, Path(id): Path<String>) -> Json<Value> {
    match state.manager.unkeep(&id) {
        Ok(_) => Json(serde_json::json!({
            "status": "ok",
            "data": { "id": id, "retain_on_exit": false },
            "error": null
        })),
        Err(e) => Json(serde_json::json!({
            "status": "error",
            "data": null,
            "error": e.to_string()
        })),
    }
}

/// DELETE /api/commands/:id
/// Purge a retained (exited) command from the manager.
/// This permanently discards the VTTY buffer and all associated state.
pub async fn purge_command(State(state): State<AppState>, Path(id): Path<String>) -> Json<Value> {
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

/// GET /api/snapshot
///
/// Combined endpoint for fast initial page load. Returns the commands list,
/// the first alive command's VTTY HTML, and resource usage for all alive
/// commands — all in a single HTTP request. This eliminates the need for
/// the client to make 2+ sequential requests on first load.
pub async fn get_snapshot(State(state): State<AppState>) -> Json<Value> {
    let commands = state.manager.list();

    // Build the commands list (same as list_commands)
    let mut data: Vec<Value> = Vec::new();
    let mut first_alive_id: Option<String> = None;

    for (idx, (id, name, args, pid, certificate)) in commands.iter().enumerate() {
        let exit_info = state
            .manager
            .get(id)
            .map(|h| {
                serde_json::json!({
                    "on_exit": h.exit_config.on_exit.as_deref().unwrap_or(""),
                    "on_error": h.exit_config.on_error.as_deref().unwrap_or(""),
                    "exit_timeout": h.exit_config.timeout_secs,
                    "retain_on_exit": h.exit_config.retain_on_exit,
                    "snapshot_on_exit": h.exit_config.snapshot_on_exit,
                })
            })
            .unwrap_or(serde_json::json!(null));
        let (alive, runtime_secs, exit_code, exit_time, frozen) = state
            .manager
            .get(id)
            .map(|h| {
                let ec = h.exit_code.lock().ok().and_then(|c| *c);
                let et = h
                    .exit_time
                    .lock()
                    .ok()
                    .and_then(|guard| *guard)
                    .map(|t| t.elapsed().as_secs());
                (h.is_alive(), h.runtime_secs(), ec, et, h.is_frozen())
            })
            .unwrap_or((false, 0.0, None, None, false));

        if alive && first_alive_id.is_none() {
            first_alive_id = Some(id.clone());
        }

        data.push(serde_json::json!({
            "id": id,
            "name": name,
            "args": args,
            "pid": pid,
            "alive": alive,
            "frozen": frozen,
            "runtime_secs": runtime_secs,
            "exit_code": exit_code,
            "exit_time_secs": exit_time,
            "status": if frozen { "frozen" } else if alive { "running" } else { "exited" },
            "certificate": certificate,
            "exit": exit_info,
            "spawn_order": idx,
        }));
    }

    // Fetch VTTY HTML + metadata for the first alive command in ONE read lock.
    let mut vtty = serde_json::json!(null);
    if let Some(ref id) = first_alive_id {
        if let Some(handle) = state.manager.get(id) {
            // Acquire read lock ONCE for HTML + all metadata (cursor, dims, etc.)
            // This replaces 10 separate emulator.read().await calls.
            let html = handle.vtty_html().await;
            let meta = handle.vtty_metadata().await;
            vtty = serde_json::json!({
                "id": id,
                "html": html,
                "cursor": { "row": meta.cursor.0, "col": meta.cursor.1 },
                "dimensions": { "rows": meta.dimensions.0, "cols": meta.dimensions.1 },
                "scrollback_lines": meta.scrollback_lines,
                "alternate_screen": meta.alternate_screen,
                "cursor_visible": meta.cursor_visible,
                "mouse_tracking": meta.mouse_tracking,
                "mouse_sgr": meta.mouse_sgr,
                "generation": meta.generation,
            });
        }
    }

    // Fetch resources for all alive commands
    let mut resources = serde_json::json!({});
    for (id, _, _, pid, _) in &commands {
        if let Some(handle) = state.manager.get(id) {
            if handle.is_alive() {
                let result = read_proc_stats(*pid);
                resources[id] = serde_json::json!({
                    "pid": pid,
                    "cpu_percent": result.cpu_percent,
                    "memory_mb": result.memory_mb,
                    "threads": result.threads,
                    "alive": true,
                });
            }
        }
    }

    Json(serde_json::json!({
        "status": "ok",
        "data": {
            "commands": data,
            "vtty": vtty,
            "resources": resources,
        },
        "error": null
    }))
}

/// Tab completion for the spawn form command input.
/// Scans the local PATH directories for executables whose basename
/// starts with the given prefix. Returns up to 50 matches.
/// Query parameter: ?prefix=hto  →  ["htop", ...]
pub async fn tab_complete(
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Json<Value> {
    use std::path::PathBuf;
    use std::fs;

    let prefix = params.get("prefix").cloned().unwrap_or_default();
    let prefix_lower = prefix.to_lowercase();

    // Get PATH from environment (server-side PATH)
    let path_dirs: Vec<PathBuf> = if let Ok(path_var) = std::env::var("PATH") {
        std::env::split_paths(&path_var).collect()
    } else {
        // Fallback to common PATH locations
        vec![
            PathBuf::from("/usr/local/bin"),
            PathBuf::from("/usr/bin"),
            PathBuf::from("/bin"),
            PathBuf::from("/usr/local/sbin"),
            PathBuf::from("/usr/sbin"),
            PathBuf::from("/sbin"),
        ]
    };

    let mut matches: Vec<String> = Vec::new();

    for dir in &path_dirs {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                // Skip directories, only consider files
                if !path.is_file() {
                    continue;
                }
                // Check if the file is executable (any execute bit set)
                use std::os::unix::fs::PermissionsExt;
                if let Ok(metadata) = path.metadata() {
                    if metadata.permissions().mode() & 0o111 == 0 {
                        continue;
                    }
                } else {
                    continue;
                }
                // Get the basename and check prefix match
                if let Some(name) = path.file_name() {
                    let name_str = name.to_string_lossy();
                    if !name_str.to_lowercase().starts_with(&prefix_lower) {
                        continue;
                    }
                    // Skip if we already have this name (from an earlier PATH dir)
                    if !matches.contains(&name_str.to_string()) {
                        matches.push(name_str.to_string());
                        if matches.len() >= 50 {
                            break;
                        }
                    }
                }
            }
            if matches.len() >= 50 {
                break;
            }
        }
    }

    matches.sort();

    Json(serde_json::json!({
        "status": "ok",
        "data": matches,
        "error": null
    }))
}
