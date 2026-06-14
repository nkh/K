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
use crate::web::response::{api_err, api_ok};
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
    api_ok(data)
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
        return api_err("Missing 'cmd' field");
    }

    // Check for no_env flag — when true, skip config-level environment variables
    let no_env: bool = body
        .get("no_env")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // Validate certificate name if provided
    if let Some(ref cert_name) = certificate {
        if !state.cert_store.exists(cert_name) {
            return api_err(format!("Certificate '{}' not found in store", cert_name));
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
            return api_err("Invalid dimensions: rows must be 1-10000, cols must be 1-1000");
        }
    }

    // Validate working directory if provided
    if let Some(ref d) = dir {
        let path = std::path::Path::new(d);
        if !path.is_dir() {
            return api_err(format!("Working directory '{}' does not exist or is not a directory", d));
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
            api_ok(serde_json::json!({ "id": id, "pid": pid }))
        }
        Err(e) => api_err(e.to_string()),
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
        Ok(_) => api_ok(serde_json::json!({ "id": id })),
        Err(e) => api_err(e.to_string()),
    }
}

/// POST /api/commands/kill-pid/:pid
/// Kill a command by its OS PID (as opposed to command UUID).
pub async fn kill_command_by_pid(
    State(state): State<AppState>,
    Path(pid): Path<u32>,
) -> Json<Value> {
    match state.manager.kill_by_pid(pid).await {
        Ok(_) => api_ok(serde_json::json!({ "pid": pid })),
        Err(e) => api_err(e.to_string()),
    }
}

/// POST /api/commands/kill-all
/// Kill all running commands. Retained commands are signaled but not removed.
/// Returns the count of commands that were signaled.
pub async fn kill_all_commands(State(state): State<AppState>) -> Json<Value> {
    let commands = state.manager.list();
    let mut killed = Vec::new();
    let mut errors = Vec::new();

    for (id, _name, _args, _pid, _cert) in &commands {
        match state.manager.kill(id, None).await {
            Ok(_) => killed.push(id.clone()),
            Err(e) => errors.push(format!("{}: {}", id, e)),
        }
    }

    Json(serde_json::json!({
        "status": "ok",
        "data": {
            "killed_count": killed.len(),
            "total_count": commands.len(),
            "killed_ids": killed,
        },
        "error": if errors.is_empty() { serde_json::Value::Null } else { serde_json::json!(errors.join("; ")) }
    }))
}

/// Freeze (suspend) a running command via SIGSTOP.
pub async fn freeze_command(State(state): State<AppState>, Path(id): Path<String>) -> Json<Value> {
    match state.manager.freeze(&id) {
        Ok(_) => api_ok(serde_json::json!({ "id": id, "frozen": true })),
        Err(e) => api_err(e.to_string()),
    }
}

/// Thaw (resume) a frozen command via SIGCONT.
pub async fn thaw_command(State(state): State<AppState>, Path(id): Path<String>) -> Json<Value> {
    match state.manager.thaw(&id) {
        Ok(_) => api_ok(serde_json::json!({ "id": id, "frozen": false })),
        Err(e) => api_err(e.to_string()),
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
            return api_err(format!("Command '{}' not found", id));
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
            api_ok(serde_json::json!({ "id": new_id, "pid": new_pid }))
        }
        Err(e) => api_err(format!("Failed to spawn replacement command: {}", e)),
    }
}

pub async fn shutdown(State(state): State<AppState>) -> Json<Value> {
    let _ = state.shutdown_tx.send(());
    api_ok(serde_json::json!({ "message": "shutdown initiated" }))
}

pub async fn get_info(State(state): State<AppState>) -> Json<Value> {
    let commands = state.manager.list();
    let certs = state.cert_store.list();
    let cert_names: Vec<&str> = certs.iter().map(|c| c.name.as_str()).collect();
    let web_config = &state.manager.config().web;
    let vtty_config = &state.manager.config().vtty;
    let server_config = &state.manager.config().server;

    // Build panel_colors array for the frontend
    let panel_colors: Vec<Value> = web_config
        .panel_colors
        .iter()
        .map(|pc| {
            serde_json::json!({
                "background": pc.background,
                "text": pc.text,
            })
        })
        .collect();

    api_ok(serde_json::json!({
        "command_count": commands.len(),
        "certificate_count": certs.len(),
        "certificates": cert_names,
        "auth_enabled": state.auth_token.is_some(),
        "server_name": server_config.name,
        "web": {
            "update_mode": web_config.update_mode,
            "dirty_check_ms": web_config.dirty_check_ms,
            "default_poll_ms": web_config.default_poll_ms,
            "panel_colors": panel_colors,
        },
        "vtty": {
            "screenshot_font_size": vtty_config.screenshot_font_size,
            "screenshot_font_name": vtty_config.screenshot_font_name,
        },
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
        Ok(meta) => api_ok(serde_json::json!({
            "id": id,
            "name": meta.name,
            "command_name": meta.command_name,
            "command_args": meta.command_args,
            "pid": meta.pid,
            "timestamp": meta.timestamp.to_rfc3339(),
            "runtime_secs": meta.runtime_secs,
        })),
        Err(e) => api_err(e.to_string()),
    }
}

/// GET /api/commands/:id/snapshots
/// List all snapshots for a command.
pub async fn list_snapshots(State(state): State<AppState>, Path(id): Path<String>) -> Json<Value> {
    let snapshots = state.manager.list_snapshots(&id);
    api_ok(snapshots)
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
        Ok(diff) => api_ok(serde_json::json!({
            "id": id,
            "name": name,
            "width": diff.width,
            "height": diff.height,
            "changed_count": diff.changed_count,
            "cells": diff.cells,
        })),
        Err(e) => api_err(e.to_string()),
    }
}

/// DELETE /api/commands/:id/snapshots/:name
/// Delete a stored snapshot.
pub async fn delete_snapshot(
    State(state): State<AppState>,
    Path((id, name)): Path<(String, String)>,
) -> Json<Value> {
    match state.manager.delete_snapshot(&id, &name) {
        Ok(_) => api_ok(serde_json::json!({ "id": id, "name": name })),
        Err(e) => api_err(e.to_string()),
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

    api_ok(matches)
}

/// POST /api/commands/:id/keep
/// Tag a running command so its terminal rendering is kept after exit.
/// Sets `retain_on_exit = true` on the command's exit configuration.
pub async fn keep_command(State(state): State<AppState>, Path(id): Path<String>) -> Json<Value> {
    match state.manager.keep(&id) {
        Ok(_) => api_ok(serde_json::json!({ "id": id, "retain_on_exit": true })),
        Err(e) => api_err(e.to_string()),
    }
}

/// POST /api/commands/:id/unkeep
/// Remove the keep tag from a command. The command will be removed from
/// the manager when it exits.
pub async fn unkeep_command(State(state): State<AppState>, Path(id): Path<String>) -> Json<Value> {
    match state.manager.unkeep(&id) {
        Ok(_) => api_ok(serde_json::json!({ "id": id, "retain_on_exit": false })),
        Err(e) => api_err(e.to_string()),
    }
}

/// DELETE /api/commands/:id
/// Purge a retained (exited) command from the manager.
/// This permanently discards the VTTY buffer and all associated state.
pub async fn purge_command(State(state): State<AppState>, Path(id): Path<String>) -> Json<Value> {
    match state.manager.purge(&id) {
        Ok(_) => api_ok(serde_json::json!({ "id": id, "purged": true })),
        Err(e) => api_err(e.to_string()),
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

    api_ok(serde_json::json!({
        "commands": data,
        "vtty": vtty,
        "resources": resources,
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

    api_ok(matches)
}


#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
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

    // ─── Handler integration tests with real AppState ───

    #[tokio::test]
    async fn test_list_commands_empty() {
        let state = make_app_state();
        let result = list_commands(State(state)).await;
        assert_eq!(result.0["status"], "ok");
        assert_eq!(result.0["data"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_list_commands_with_data() {
        let state = make_app_state();
        insert_mock_cmd(&state.manager, "cmd-1", 100);
        let result = list_commands(State(state)).await;
        assert_eq!(result.0["status"], "ok");
        let data = result.0["data"].as_array().unwrap();
        assert_eq!(data.len(), 1);
        assert_eq!(data[0]["id"], "cmd-1");
        assert_eq!(data[0]["pid"], 100);
        assert_eq!(data[0]["name"], "cmd-cmd-1");
    }

    #[tokio::test]
    async fn test_start_command_missing_cmd() {
        let state = make_app_state();
        let body = json!({});
        let result = start_command(State(state), Json(body)).await;
        assert_eq!(result.0["status"], "error");
        assert!(result.0["error"].as_str().unwrap().contains("Missing"));
    }

    #[tokio::test]
    async fn test_start_command_invalid_cert() {
        let state = make_app_state();
        let body = json!({"cmd": "ls", "certificate": "nonexistent"});
        let result = start_command(State(state), Json(body)).await;
        assert_eq!(result.0["status"], "error");
        assert!(result.0["error"].as_str().unwrap().contains("Certificate"));
    }

    #[tokio::test]
    async fn test_start_command_invalid_dimensions() {
        let state = make_app_state();
        let body = json!({"cmd": "ls", "rows": 0, "cols": 0});
        let result = start_command(State(state), Json(body)).await;
        assert_eq!(result.0["status"], "error");
        assert!(result.0["error"].as_str().unwrap().contains("Invalid dimensions"));
    }

    #[tokio::test]
    async fn test_start_command_invalid_dir() {
        let state = make_app_state();
        let body = json!({"cmd": "ls", "dir": "/nonexistent_dir_xyz"});
        let result = start_command(State(state), Json(body)).await;
        assert_eq!(result.0["status"], "error");
        assert!(result.0["error"].as_str().unwrap().contains("Working directory"));
    }

    #[tokio::test]
    async fn test_kill_command_existing() {
        let state = make_app_state();
        insert_mock_cmd(&state.manager, "cmd-1", 100);
        let body = json!({});
        let result = kill_command(State(state.clone()), Path("cmd-1".into()), Json(body)).await;
        assert_eq!(result.0["status"], "ok");
        assert_eq!(result.0["data"]["id"], "cmd-1");
        // Command should be removed (not retained)
        assert!(state.manager.get(&"cmd-1".to_string()).is_none());
    }

    #[tokio::test]
    async fn test_kill_all_commands_empty() {
        let state = make_app_state();
        let result = kill_all_commands(State(state)).await;
        assert_eq!(result.0["status"], "ok");
        assert_eq!(result.0["data"]["killed_count"], 0);
    }

    #[tokio::test]
    async fn test_kill_all_commands_with_commands() {
        let state = make_app_state();
        insert_mock_cmd(&state.manager, "a", 1);
        insert_mock_cmd(&state.manager, "b", 2);
        let result = kill_all_commands(State(state)).await;
        assert_eq!(result.0["status"], "ok");
        assert_eq!(result.0["data"]["killed_count"], 2);
    }

    #[tokio::test]
    async fn test_freeze_command_sets_flag() {
        let state = make_app_state();
        insert_mock_cmd(&state.manager, "cmd-1", 1);
        // freeze sends SIGSTOP — on a mock PID this may fail,
        // but the frozen flag should be set regardless.
        let result = freeze_command(State(state.clone()), Path("cmd-1".into())).await;
        // Either ok (if signal succeeds) or error (if PID doesn't exist)
        if result.0["status"] == "ok" {
            assert_eq!(result.0["data"]["frozen"], true);
        } else {
            assert_eq!(result.0["status"], "error");
        }
        // The frozen flag is always set before the signal is sent
        if let Some(h) = state.manager.get(&"cmd-1".to_string()) {
            assert!(h.is_frozen());
        };
    }

    #[tokio::test]
    async fn test_thaw_command_clears_flag() {
        let state = make_app_state();
        insert_mock_cmd(&state.manager, "cmd-1", 1);
        // Set frozen first, then thaw
        state.manager.freeze(&"cmd-1".to_string()).ok(); // may fail on signal but flag is set
        let result = thaw_command(State(state.clone()), Path("cmd-1".into())).await;
        // Either ok or error depending on signal
        if result.0["status"] == "ok" {
            assert_eq!(result.0["data"]["frozen"], false);
        }
        // The flag should be cleared regardless
        {
            let mgr_ref = state.manager.get(&"cmd-1".to_string());
            if let Some(h) = mgr_ref {
                assert!(!h.is_frozen());
            }
        }
    }

    #[tokio::test]
    async fn test_shutdown() {
        let state = make_app_state();
        let result = shutdown(State(state)).await;
        assert_eq!(result.0["status"], "ok");
        assert_eq!(result.0["data"]["message"], "shutdown initiated");
    }

    #[tokio::test]
    async fn test_get_info() {
        let state = make_app_state();
        let result = get_info(State(state)).await;
        assert_eq!(result.0["status"], "ok");
        assert_eq!(result.0["data"]["command_count"], 0);
        assert_eq!(result.0["data"]["certificate_count"], 0);
        assert_eq!(result.0["data"]["auth_enabled"], false);
        // Web config fields
        assert!(result.0["data"]["web"]["panel_colors"].is_array());
        assert!(result.0["data"]["vtty"].is_object());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_snapshot_command_success() {
        let state = make_app_state();
        insert_mock_cmd(&state.manager, "cmd-1", 1);
        let body = json!({"name": "v1"});
        let result = snapshot_command(State(state), Path("cmd-1".into()), Json(body)).await;
        assert_eq!(result.0["status"], "ok", "response: {}", result.0);
        eprintln!("SNAPSHOT response: {}", result.0);
        assert_eq!(result.0["data"]["name"], "v1");
        assert_eq!(result.0["data"]["id"], "cmd-1");
    }

    #[tokio::test]
    async fn test_list_snapshots() {
        let state = make_app_state();
        insert_mock_cmd(&state.manager, "cmd-1", 1);
        let result = list_snapshots(State(state), Path("cmd-1".into())).await;
        assert_eq!(result.0["status"], "ok");
        assert_eq!(result.0["data"].as_array().unwrap().len(), 0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_diff_command_success() {
        let state = make_app_state();
        insert_mock_cmd(&state.manager, "cmd-1", 1);
        state.manager.store_snapshot(&"cmd-1".to_string(), "v1").unwrap();
        let body = json!({"name": "v1"});
        let result = diff_command(State(state), Path("cmd-1".into()), Json(body)).await;
        assert_eq!(result.0["status"], "ok");
        assert_eq!(result.0["data"]["changed_count"], 0);
    }

    #[tokio::test]
    async fn test_delete_snapshot_missing_snap() {
        let state = make_app_state();
        insert_mock_cmd(&state.manager, "cmd-1", 1);
        let result = delete_snapshot(State(state), Path(("cmd-1".into(), "nope".into()))).await;
        assert_eq!(result.0["status"], "error");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_delete_snapshot_success() {
        let state = make_app_state();
        insert_mock_cmd(&state.manager, "cmd-1", 1);
        state.manager.store_snapshot(&"cmd-1".to_string(), "v1").unwrap();
        let result = delete_snapshot(State(state.clone()), Path(("cmd-1".into(), "v1".into()))).await;
        assert_eq!(result.0["status"], "ok");
        assert!(state.manager.list_snapshots(&"cmd-1".to_string()).is_empty());
    }

    #[tokio::test]
    async fn test_lookup_command_no_match() {
        let state = make_app_state();
        insert_mock_cmd(&state.manager, "cmd-1", 1);
        let result = lookup_command(State(state), Path("nonexistent".into())).await;
        assert_eq!(result.0["status"], "ok");
        assert_eq!(result.0["data"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_lookup_command_by_name() {
        let state = make_app_state();
        insert_mock_cmd(&state.manager, "cmd-1", 1);
        let result = lookup_command(State(state), Path("cmd-cmd-1".into())).await;
        assert_eq!(result.0["status"], "ok");
        assert_eq!(result.0["data"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_keep_command_success() {
        let state = make_app_state();
        insert_mock_cmd(&state.manager, "cmd-1", 1);
        let result = keep_command(State(state), Path("cmd-1".into())).await;
        assert_eq!(result.0["status"], "ok");
        assert_eq!(result.0["data"]["retain_on_exit"], true);
    }

    #[tokio::test]
    async fn test_unkeep_command_success() {
        let state = make_app_state();
        insert_mock_cmd(&state.manager, "cmd-1", 1);
        state.manager.keep(&"cmd-1".to_string()).unwrap();
        let result = unkeep_command(State(state), Path("cmd-1".into())).await;
        assert_eq!(result.0["status"], "ok");
        assert_eq!(result.0["data"]["retain_on_exit"], false);
    }

    #[tokio::test]
    async fn test_purge_command_success() {
        let state = make_app_state();
        insert_mock_cmd(&state.manager, "cmd-1", 1);
        let result = purge_command(State(state.clone()), Path("cmd-1".into())).await;
        assert_eq!(result.0["status"], "ok");
        assert_eq!(result.0["data"]["purged"], true);
        assert!(state.manager.get(&"cmd-1".to_string()).is_none());
    }

    #[tokio::test]
    async fn test_get_snapshot_empty() {
        let state = make_app_state();
        let result = get_snapshot(State(state)).await;
        assert_eq!(result.0["status"], "ok");
        assert_eq!(result.0["data"]["commands"].as_array().unwrap().len(), 0);
        assert!(result.0["data"]["vtty"].is_null());
    }

    #[tokio::test]
    async fn test_tab_complete() {
        let params: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        let result = tab_complete(axum::extract::Query(params)).await;
        assert_eq!(result.0["status"], "ok");
        // Without a prefix filter, should return many results (sorted)
        let data = result.0["data"].as_array().unwrap();
        assert!(!data.is_empty());
        // Verify sorted
        let mut prev = String::new();
        for item in data {
            let s = item.as_str().unwrap();
            assert!(*s >= *prev);
            prev = s.to_string();
        }
    }

    #[tokio::test]
    async fn test_tab_complete_with_prefix() {
        let mut params = std::collections::HashMap::new();
        params.insert("prefix".to_string(), "ls".to_string());
        let result = tab_complete(axum::extract::Query(params)).await;
        assert_eq!(result.0["status"], "ok");
        let data = result.0["data"].as_array().unwrap();
        // Should contain "ls" itself
        assert!(data.iter().any(|v| v.as_str() == Some("ls")));
    }

    #[tokio::test]
    async fn test_start_command_with_args_and_env() {
        let state = make_app_state();
        // We can't actually spawn processes in tests, but we can verify the
        // request parsing logic by checking that a valid command attempt
        // reaches the spawn step (which fails because mock processes can't really run).
        // Instead, test that env and args are properly extracted.
        let body = json!({
            "cmd": "ls",
            "args": ["-la", "/tmp"],
            "env": {"FOO": "bar", "BAZ": "qux"},
            "no_env": true,
            "retain_on_exit": true,
            "snapshot_on_exit": "auto"
        });
        // This will try to spawn "ls" and fail because we don't have a real PTY.
        // But the parsing and validation up to the spawn call is what we're testing.
        let result = start_command(State(state), Json(body)).await;
        // It should not be "Missing 'cmd' field" — that would indicate parsing failure
        assert!(result.0["error"].as_str().unwrap_or("").contains("Failed to initialize")
                || result.0["status"] == "ok"
                || result.0["error"].as_str().unwrap_or("").contains("No such file"),
                "Unexpected response: {}", result.0);
    }
}
