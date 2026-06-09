#![cfg(feature = "vrw")]
#![allow(dead_code, unused_imports)]
use anyhow::Result;

use crate::cli::args::Cli;
use crate::instance::registry::InstanceRegistry;

use super::common::{build_full_display_string, collect_all_commands, http_client, instance_url};

/// Purge a retained (exited) command by ID or name on any running instance.
///
/// Only exited commands (alive == false) are considered.
/// If no target is given and exactly one exited command exists, it is purged.
/// If the target matches a command ID (prefix), that command is purged.
/// Otherwise, matching proceeds by name (same three-round strategy as stop).
///
/// Returns true if exactly one exited command was found and purged.
pub async fn handle_purge_command(_cli: &Cli, target: Option<&str>, interactive: bool) -> Result<bool> {
    let registry = InstanceRegistry::new()?;
    let instances = registry.list_instances();

    if instances.is_empty() {
        return Ok(false);
    }

    let client = http_client();

    // Collect all *exited* commands from all instances.
    let all_commands: Vec<(u32, String, u32, String, String)> = {
        let _all = collect_all_commands(&client, &instances).await;
        // Re-fetch with alive status to filter
        let mut exited = Vec::new();
        for info in &instances {
            let url = instance_url(info, &None);
            if let Ok(resp) = client.get(format!("{}/api/commands", url)).send().await {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    if let Some(cmds) = json["data"].as_array() {
                        for cmd in cmds {
                            let alive = cmd.get("alive").and_then(|v| v.as_bool()).unwrap_or(true);
                            if alive {
                                continue;
                            }
                            let cmd_pid =
                                cmd.get("pid").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                            if let Some(id) = cmd.get("id").and_then(|v| v.as_str()) {
                                let (name, full) = build_full_display_string(cmd);
                                exited.push((info.pid, id.to_string(), cmd_pid, name, full));
                            }
                        }
                    }
                }
            }
        }
        exited
    };

    // No target: purge the only exited command if there is exactly one,
    // or use interactive selection if -i was given.
    let target = match target {
        Some(t) => t,
        None => match all_commands.len() {
            0 => {
                tracing::warn!("No exited commands to purge.");
                return Ok(false);
            }
            1 => {
                let (inst_pid, ref cmd_id, _cmd_pid, _, ref full) = all_commands[0];
                let info = instances.iter().find(|i| i.pid == inst_pid).unwrap();
                let url = instance_url(info, &None);
                tracing::info!("Purging only exited command: {} (ID {})", full, cmd_id);
                return purge_command_by_id(&client, &url, cmd_id, full).await;
            }
            _ => {
                if interactive {
                    let items: Vec<_> = all_commands
                        .iter()
                        .map(|(_, id, _, _, full)| crate::cli::interactive_select::SelectItem {
                            label: format!("{} — {}", &id[..8.min(id.len())], full),
                            id: id.clone(),
                        })
                        .collect();
                    let selected = crate::cli::interactive_select::select_items(
                        &items, "Select exited commands to purge [space-separated numbers]",
                    )?;
                    let mut purged_any = false;
                    for item in &selected {
                        let cmd_id = &item.id;
                        let entry = all_commands.iter().find(|(_, id, _, _, _)| id == cmd_id);
                        if let Some((inst_pid, _, _, _, full)) = entry {
                            let info = instances.iter().find(|i| i.pid == *inst_pid).unwrap();
                            let url = instance_url(info, &None);
                            if purge_command_by_id(&client, &url, cmd_id, full).await? {
                                purged_any = true;
                            }
                        }
                    }
                    return Ok(purged_any);
                }
                tracing::warn!("Multiple exited commands. Specify which one to purge:");
                for (_, cmd_id, _, _, full) in &all_commands {
                    tracing::warn!("  {} — {}", &cmd_id[..8.min(cmd_id.len())], full);
                }
                tracing::warn!("Usage: vrw purge <ID or name>");
                return Ok(false);
            }
        },
    };

    // Fast path: if target matches a command ID prefix.
    let id_matches: Vec<_> = all_commands
        .iter()
        .filter(|(_, id, _, _, _)| id.starts_with(target))
        .collect();
    if id_matches.len() == 1 {
        let (inst_pid, ref cmd_id, _, _, ref full) = id_matches[0];
        let info = instances.iter().find(|i| i.pid == *inst_pid).unwrap();
        let url = instance_url(info, &None);
        return purge_command_by_id(&client, &url, cmd_id, full).await;
    }
    if id_matches.len() > 1 {
        tracing::warn!("Multiple exited commands match ID prefix '{}':", target);
        for (_, cmd_id, _, _, full) in &id_matches {
            tracing::warn!("  {} — {}", cmd_id, full);
        }
        return Ok(false);
    }

    // Name matching: same three-round strategy as stop_command.
    let exact: Vec<_> = all_commands
        .iter()
        .filter(|(_, _, _, name, full)| name == target || full == target)
        .collect();

    if exact.len() == 1 {
        let (inst_pid, ref cmd_id, _, _, _) = exact[0];
        let info = instances.iter().find(|i| i.pid == *inst_pid).unwrap();
        let url = instance_url(info, &None);
        return purge_command_by_id(&client, &url, cmd_id, target).await;
    }
    if exact.len() > 1 {
        tracing::warn!("Multiple exited commands match '{}':", target);
        for (_, cmd_id, _, _, full) in &exact {
            tracing::warn!("  {} — {}", cmd_id, full);
        }
        return Ok(false);
    }

    // Prefix match on full string
    let prefix_full: Vec<_> = all_commands
        .iter()
        .filter(|(_, _, _, _, full)| full.starts_with(target))
        .collect();

    if prefix_full.len() == 1 {
        let (inst_pid, ref cmd_id, _, _, full) = prefix_full[0];
        let info = instances.iter().find(|i| i.pid == *inst_pid).unwrap();
        let url = instance_url(info, &None);
        return purge_command_by_id(&client, &url, cmd_id, full).await;
    }
    if prefix_full.len() > 1 {
        tracing::warn!("Multiple exited commands match prefix '{}':", target);
        for (_, cmd_id, _, _, full) in &prefix_full {
            tracing::warn!("  {} — {}", cmd_id, full);
        }
        return Ok(false);
    }

    // Prefix match on name only
    let prefix_name: Vec<_> = all_commands
        .iter()
        .filter(|(_, _, _, name, _)| name.starts_with(target))
        .collect();

    if prefix_name.len() == 1 {
        let (inst_pid, ref cmd_id, _, _, full) = prefix_name[0];
        let info = instances.iter().find(|i| i.pid == *inst_pid).unwrap();
        let url = instance_url(info, &None);
        return purge_command_by_id(&client, &url, cmd_id, full).await;
    }
    if prefix_name.len() > 1 {
        tracing::warn!("Multiple exited commands match name prefix '{}':", target);
        for (_, cmd_id, _, _, full) in &prefix_name {
            tracing::warn!("  {} — {}", cmd_id, full);
        }
        return Ok(false);
    }

    tracing::warn!("No exited command matching '{}' found.", target);
    Ok(false)
}

/// Purge a specific command by ID via DELETE /api/commands/:id.
pub(crate) async fn purge_command_by_id(
    client: &reqwest::Client,
    url: &str,
    cmd_id: &str,
    label: &str,
) -> Result<bool> {
    let resp = client
        .delete(format!("{}/api/commands/{}", url, cmd_id))
        .send()
        .await;

    match resp {
        Ok(resp) => {
            let status = resp.status();
            let body: serde_json::Value = resp
                .json()
                .await
                .unwrap_or(serde_json::json!({"status": "unknown"}));
            if status.is_success() && body.get("status").and_then(|s| s.as_str()) == Some("ok") {
                println!("Purged: {}", label);
                Ok(true)
            } else {
                let err_msg = body
                    .get("error")
                    .and_then(|e| e.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| format!("HTTP {}", status));
                tracing::error!("Failed to purge '{}': {}", label, err_msg);
                Ok(false)
            }
        }
        Err(e) => {
            tracing::error!("Failed to purge '{}': {}", label, e);
            Ok(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;
    use super::*;
    use crate::cli::commands::common::http_client;

    #[test]
    fn test_purge_command_by_id_callable() {
        let _ = purge_command_by_id;
    }

    #[test]
    fn test_handle_purge_command_callable() {
        let _ = handle_purge_command;
    }

    #[tokio::test]
    async fn test_purge_command_by_id_connection_refused() {
        let client = http_client();
        // Connecting to a non-existent server should return Ok(false)
        let result = purge_command_by_id(&client, "http://127.0.0.1:1", "fake-id", "test-label").await;
        assert!(result.is_ok());
        assert!(!result.unwrap(), "connection refused should return false");
    }

    #[tokio::test]
    async fn test_handle_purge_command_no_instances() {
        let cli = crate::cli::args::Cli::try_parse_from(["vrw", "purge"]).unwrap();
        let result = handle_purge_command(&cli, None, false).await;
        assert!(result.is_ok());
        assert!(!result.unwrap(), "no instances should return false");
    }
}

