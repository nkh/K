#![cfg(feature = "vrw")]
#![allow(dead_code, unused_imports)]
use anyhow::Result;

use crate::cli::args::Cli;
use crate::instance::registry::InstanceRegistry;

use super::common::{build_full_display_string, http_client, instance_url, CommandTarget, resolve_target_command};

/// Tag a running command to retain its VTTY buffer after exit.
///
/// Only running commands (alive == true) are considered for `keep`.
/// For `unkeep`, all commands with `retain_on_exit == true` are shown.
///
/// Returns true if exactly one command was found and tagged.
pub async fn handle_keep_command(_cli: &Cli, target: Option<&str>, interactive: bool) -> Result<bool> {
    let registry = InstanceRegistry::new()?;
    let instances = registry.list_instances();

    if instances.is_empty() {
        return Ok(false);
    }

    let client = http_client();

    // Collect all *running* commands from all instances.
    let all_commands = collect_running_commands(&client, &instances).await;

    // When target is specified, try resolve_target_command (handles PID, name, prefix).
    // On failure, fall through to interactive picker if enabled, or return Ok(false)
    // so the dispatch layer prints the standard error message.
    let resolved = if target.is_some() {
        resolve_target_command(target, &all_commands, "No running command").ok()
    } else if all_commands.len() == 1 {
        Some(all_commands[0].clone())
    } else {
        None
    };

    if let Some((inst_pid, cmd_id, _cmd_pid, _, full)) = resolved {
        let info = instances.iter().find(|i| i.pid == inst_pid).unwrap();
        let url = instance_url(info, &None);
        return keep_command_by_id(&client, &url, &cmd_id, &full).await;
    }

    // No match or no target — try interactive or report failure
    if interactive {
        let items: Vec<_> = all_commands
            .iter()
            .map(|(_, id, _, _, full)| crate::cli::interactive_select::SelectItem {
                label: format!("{} — {}", &id[..8.min(id.len())], full),
                id: id.clone(),
            })
            .collect();
        if items.is_empty() {
            tracing::warn!("No running commands to keep.");
            return Ok(false);
        }
        let selected = crate::cli::interactive_select::select_items(
            &items, "Select commands to keep [space-separated numbers]",
        )?;
        let mut kept_any = false;
        for item in &selected {
            let cmd_id = &item.id;
            let entry = all_commands.iter().find(|(_, id, _, _, _)| id == cmd_id);
            if let Some((inst_pid, _, _, _, full)) = entry {
                let info = instances.iter().find(|i| i.pid == *inst_pid).unwrap();
                let url = instance_url(info, &None);
                if keep_command_by_id(&client, &url, cmd_id, full).await? {
                    kept_any = true;
                }
            }
        }
        return Ok(kept_any);
    }

    Ok(false)
}

/// Unkeep: remove retain_on_exit tag from commands.
pub async fn handle_unkeep_command(_cli: &Cli, target: Option<&str>, interactive: bool) -> Result<bool> {
    let registry = InstanceRegistry::new()?;
    let instances = registry.list_instances();

    if instances.is_empty() {
        return Ok(false);
    }

    let client = http_client();

    // Collect all commands with retain_on_exit == true.
    let all_commands = collect_kept_commands(&client, &instances).await;

    let resolved = if target.is_some() {
        resolve_target_command(target, &all_commands, "No kept command").ok()
    } else if all_commands.len() == 1 {
        Some(all_commands[0].clone())
    } else {
        None
    };

    if let Some((inst_pid, cmd_id, _cmd_pid, _, full)) = resolved {
        let info = instances.iter().find(|i| i.pid == inst_pid).unwrap();
        let url = instance_url(info, &None);
        return unkeep_command_by_id(&client, &url, &cmd_id, &full).await;
    }

    // No match or no target — try interactive or report failure
    if interactive {
        let items: Vec<_> = all_commands
            .iter()
            .map(|(_, id, _, _, full)| crate::cli::interactive_select::SelectItem {
                label: format!("{} — {}", &id[..8.min(id.len())], full),
                id: id.clone(),
            })
            .collect();
        if items.is_empty() {
            tracing::warn!("No kept commands to unkeep.");
            return Ok(false);
        }
        let selected = crate::cli::interactive_select::select_items(
            &items, "Select commands to unkeep [space-separated numbers]",
        )?;
        let mut unkept_any = false;
        for item in &selected {
            let cmd_id = &item.id;
            let entry = all_commands.iter().find(|(_, id, _, _, _)| id == cmd_id);
            if let Some((inst_pid, _, _, _, full)) = entry {
                let info = instances.iter().find(|i| i.pid == *inst_pid).unwrap();
                let url = instance_url(info, &None);
                if unkeep_command_by_id(&client, &url, cmd_id, full).await? {
                    unkept_any = true;
                }
            }
        }
        return Ok(unkept_any);
    }

    Ok(false)
}

/// Collect all running (alive) commands from all instances.
async fn collect_running_commands(
    client: &reqwest::Client,
    instances: &[crate::instance::info::InstanceInfo],
) -> Vec<CommandTarget> {
    let mut running = Vec::new();
    for info in instances {
        let url = instance_url(info, &None);
        if let Ok(resp) = client.get(format!("{}/api/commands", url)).send().await {
            if let Ok(json) = resp.json::<serde_json::Value>().await {
                if let Some(cmds) = json["data"].as_array() {
                    for cmd in cmds {
                        let alive = cmd.get("alive").and_then(|v| v.as_bool()).unwrap_or(false);
                        if !alive {
                            continue;
                        }
                        let cmd_pid = cmd.get("pid").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                        if let Some(id) = cmd.get("id").and_then(|v| v.as_str()) {
                            let (name, full) = build_full_display_string(cmd);
                            running.push((info.pid, id.to_string(), cmd_pid, name, full));
                        }
                    }
                }
            }
        }
    }
    running
}

/// Collect all commands with retain_on_exit == true from all instances.
async fn collect_kept_commands(
    client: &reqwest::Client,
    instances: &[crate::instance::info::InstanceInfo],
) -> Vec<CommandTarget> {
    let mut kept = Vec::new();
    for info in instances {
        let url = instance_url(info, &None);
        if let Ok(resp) = client.get(format!("{}/api/commands", url)).send().await {
            if let Ok(json) = resp.json::<serde_json::Value>().await {
                if let Some(cmds) = json["data"].as_array() {
                    for cmd in cmds {
                        let retain = cmd
                            .get("exit")
                            .and_then(|e| e.get("retain_on_exit"))
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        if !retain {
                            continue;
                        }
                        let cmd_pid = cmd.get("pid").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                        if let Some(id) = cmd.get("id").and_then(|v| v.as_str()) {
                            let (name, full) = build_full_display_string(cmd);
                            kept.push((info.pid, id.to_string(), cmd_pid, name, full));
                        }
                    }
                }
            }
        }
    }
    kept
}

async fn keep_command_by_id(
    client: &reqwest::Client,
    url: &str,
    cmd_id: &str,
    label: &str,
) -> Result<bool> {
    let resp = client
        .post(format!("{}/api/commands/{}/keep", url, cmd_id))
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
                println!("Kept: {} (terminal rendering retained after exit)", label);
                Ok(true)
            } else {
                let err_msg = body
                    .get("error")
                    .and_then(|e| e.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| format!("HTTP {}", status));
                tracing::error!("Failed to keep '{}': {}", label, err_msg);
                Ok(false)
            }
        }
        Err(e) => {
            tracing::error!("Failed to keep '{}': {}", label, e);
            Ok(false)
        }
    }
}

async fn unkeep_command_by_id(
    client: &reqwest::Client,
    url: &str,
    cmd_id: &str,
    label: &str,
) -> Result<bool> {
    let resp = client
        .post(format!("{}/api/commands/{}/unkeep", url, cmd_id))
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
                println!("Unkept: {} (will be removed on exit)", label);
                Ok(true)
            } else {
                let err_msg = body
                    .get("error")
                    .and_then(|e| e.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| format!("HTTP {}", status));
                tracing::error!("Failed to unkeep '{}': {}", label, err_msg);
                Ok(false)
            }
        }
        Err(e) => {
            tracing::error!("Failed to unkeep '{}': {}", label, e);
            Ok(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;
    use super::*;
    use crate::cli::commands::common::http_client;

    #[tokio::test]
    async fn test_keep_command_by_id_connection_refused() {
        let client = http_client();
        let result = keep_command_by_id(&client, "http://127.0.0.1:1", "fake-id", "test").await;
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[tokio::test]
    async fn test_unkeep_command_by_id_connection_refused() {
        let client = http_client();
        let result = unkeep_command_by_id(&client, "http://127.0.0.1:1", "fake-id", "test").await;
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[tokio::test]
    async fn test_handle_keep_command_no_instances() {
        let cli = crate::cli::args::Cli::try_parse_from(["vrw", "keep"]).unwrap();
        let result = handle_keep_command(&cli, None, false).await;
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[tokio::test]
    async fn test_handle_unkeep_command_no_instances() {
        let cli = crate::cli::args::Cli::try_parse_from(["vrw", "unkeep"]).unwrap();
        let result = handle_unkeep_command(&cli, None, false).await;
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }
}

