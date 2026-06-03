#![cfg(feature = "vrw")]
#![allow(dead_code, unused_imports)]
use anyhow::Result;

use crate::cli::args::Cli;
use crate::instance::registry::InstanceRegistry;

use super::common::{build_full_display_string, collect_all_commands, http_client, instance_url};

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
    let all_commands: Vec<(u32, String, u32, String, String)> = {
        let mut running = Vec::new();
        for info in &instances {
            let url = instance_url(info, &None);
            if let Ok(resp) = client.get(format!("{}/api/commands", url)).send().await {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    if let Some(cmds) = json["data"].as_array() {
                        for cmd in cmds {
                            let alive = cmd.get("alive").and_then(|v| v.as_bool()).unwrap_or(false);
                            if !alive {
                                continue;
                            }
                            let cmd_pid =
                                cmd.get("pid").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
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
    };

    let target = match target {
        Some(t) => t,
        None => match all_commands.len() {
            0 => {
                tracing::warn!("No running commands to keep.");
                return Ok(false);
            }
            1 => {
                let (inst_pid, ref cmd_id, _cmd_pid, _, ref full) = all_commands[0];
                let info = instances.iter().find(|i| i.pid == inst_pid).unwrap();
                let url = instance_url(info, &None);
                tracing::info!("Keeping only running command: {} (ID {})", full, cmd_id);
                return keep_command_by_id(&client, &url, cmd_id, full).await;
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
                tracing::warn!("Multiple running commands. Specify which one to keep:");
                for (_, cmd_id, _, _, full) in &all_commands {
                    tracing::warn!("  {} — {}", &cmd_id[..8.min(cmd_id.len())], full);
                }
                tracing::warn!("Usage: vrw keep <ID or name>");
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
        return keep_command_by_id(&client, &url, cmd_id, full).await;
    }
    if id_matches.len() > 1 {
        tracing::warn!("Multiple commands match ID prefix '{}':", target);
        for (_, cmd_id, _, _, full) in &id_matches {
            tracing::warn!("  {} — {}", cmd_id, full);
        }
        return Ok(false);
    }

    // Name matching
    let exact: Vec<_> = all_commands
        .iter()
        .filter(|(_, _, _, name, full)| name == target || full == target)
        .collect();

    if exact.len() == 1 {
        let (inst_pid, ref cmd_id, _, _, _) = exact[0];
        let info = instances.iter().find(|i| i.pid == *inst_pid).unwrap();
        let url = instance_url(info, &None);
        return keep_command_by_id(&client, &url, cmd_id, target).await;
    }
    if exact.len() > 1 {
        tracing::warn!("Multiple commands match '{}':", target);
        for (_, cmd_id, _, _, full) in &exact {
            tracing::warn!("  {} — {}", cmd_id, full);
        }
        return Ok(false);
    }

    let prefix_full: Vec<_> = all_commands
        .iter()
        .filter(|(_, _, _, _, full)| full.starts_with(target))
        .collect();

    if prefix_full.len() == 1 {
        let (inst_pid, ref cmd_id, _, _, full) = prefix_full[0];
        let info = instances.iter().find(|i| i.pid == *inst_pid).unwrap();
        let url = instance_url(info, &None);
        return keep_command_by_id(&client, &url, cmd_id, full).await;
    }

    let prefix_name: Vec<_> = all_commands
        .iter()
        .filter(|(_, _, _, name, _)| name.starts_with(target))
        .collect();

    if prefix_name.len() == 1 {
        let (inst_pid, ref cmd_id, _, _, full) = prefix_name[0];
        let info = instances.iter().find(|i| i.pid == *inst_pid).unwrap();
        let url = instance_url(info, &None);
        return keep_command_by_id(&client, &url, cmd_id, full).await;
    }
    if prefix_name.len() > 1 {
        tracing::warn!("Multiple commands match name prefix '{}':", target);
        for (_, cmd_id, _, _, full) in &prefix_name {
            tracing::warn!("  {} — {}", cmd_id, full);
        }
        return Ok(false);
    }

    tracing::warn!("No running command matching '{}' found.", target);
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
    let all_commands: Vec<(u32, String, u32, String, String)> = {
        let mut kept = Vec::new();
        for info in &instances {
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
                            let cmd_pid =
                                cmd.get("pid").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
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
    };

    let target = match target {
        Some(t) => t,
        None => match all_commands.len() {
            0 => {
                tracing::warn!("No kept commands to unkeep.");
                return Ok(false);
            }
            1 => {
                let (inst_pid, ref cmd_id, _cmd_pid, _, ref full) = all_commands[0];
                let info = instances.iter().find(|i| i.pid == inst_pid).unwrap();
                let url = instance_url(info, &None);
                tracing::info!("Unkeeping only kept command: {} (ID {})", full, cmd_id);
                return unkeep_command_by_id(&client, &url, cmd_id, full).await;
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
                tracing::warn!("Multiple kept commands. Specify which one to unkeep:");
                for (_, cmd_id, _, _, full) in &all_commands {
                    tracing::warn!("  {} — {}", &cmd_id[..8.min(cmd_id.len())], full);
                }
                tracing::warn!("Usage: vrw unkeep <ID or name>");
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
        return unkeep_command_by_id(&client, &url, cmd_id, full).await;
    }
    if id_matches.len() > 1 {
        tracing::warn!("Multiple kept commands match ID prefix '{}':", target);
        for (_, cmd_id, _, _, full) in &id_matches {
            tracing::warn!("  {} — {}", cmd_id, full);
        }
        return Ok(false);
    }

    // Name matching
    let exact: Vec<_> = all_commands
        .iter()
        .filter(|(_, _, _, name, full)| name == target || full == target)
        .collect();

    if exact.len() == 1 {
        let (inst_pid, ref cmd_id, _, _, _) = exact[0];
        let info = instances.iter().find(|i| i.pid == *inst_pid).unwrap();
        let url = instance_url(info, &None);
        return unkeep_command_by_id(&client, &url, cmd_id, target).await;
    }
    if exact.len() > 1 {
        tracing::warn!("Multiple kept commands match '{}':", target);
        for (_, cmd_id, _, _, full) in &exact {
            tracing::warn!("  {} — {}", cmd_id, full);
        }
        return Ok(false);
    }

    let prefix_full: Vec<_> = all_commands
        .iter()
        .filter(|(_, _, _, _, full)| full.starts_with(target))
        .collect();

    if prefix_full.len() == 1 {
        let (inst_pid, ref cmd_id, _, _, full) = prefix_full[0];
        let info = instances.iter().find(|i| i.pid == *inst_pid).unwrap();
        let url = instance_url(info, &None);
        return unkeep_command_by_id(&client, &url, cmd_id, full).await;
    }

    let prefix_name: Vec<_> = all_commands
        .iter()
        .filter(|(_, _, _, name, _)| name.starts_with(target))
        .collect();

    if prefix_name.len() == 1 {
        let (inst_pid, ref cmd_id, _, _, full) = prefix_name[0];
        let info = instances.iter().find(|i| i.pid == *inst_pid).unwrap();
        let url = instance_url(info, &None);
        return unkeep_command_by_id(&client, &url, cmd_id, full).await;
    }

    tracing::warn!("No kept command matching '{}' found.", target);
    Ok(false)
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
