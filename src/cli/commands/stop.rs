use anyhow::Result;

use crate::cli::args::Cli;
use crate::instance::info::InstanceInfo;
use crate::instance::registry::InstanceRegistry;

use super::common::{collect_all_commands, http_client, instance_url, resolve_pid_to_id};

/// Stop a specific command by PID or name on any running instance.
///
/// If `target` parses as a u32, it is treated as a PID and resolved
/// via `resolve_pid_to_id` (same as freeze/thaw).
///
/// If `target` is a name (or "name args..."), matching proceeds in three
/// rounds with increasing looseness.  A match from an earlier round wins:
///   1. Exact: `name == target` or `name arg1 arg2 ... == target`
///   2. Prefix on full: `name arg1 arg2 ...` starts with `target`
///   3. Prefix on name: `name` starts with `target`
///      If after all rounds exactly one command matches, it is stopped.
///      If multiple commands match, an error lists them and suggests using a
///      PID to disambiguate.
///
/// Returns true if exactly one command was found and stopped.
pub async fn handle_stop_command(_cli: &Cli, target: Option<&str>) -> Result<bool> {
    let registry = InstanceRegistry::new()?;
    let instances = registry.list_instances();

    if instances.is_empty() {
        return Ok(false);
    }

    let client = http_client();

    // If no target given, stop the only command if there is exactly one.
    let target = match target {
        Some(t) => t,
        None => {
            // Collect all commands from all instances
            let all_commands = collect_all_commands(&client, &instances).await;

            match all_commands.len() {
                0 => {
                    tracing::warn!("No commands running.");
                    return Ok(false);
                }
                1 => {
                    let (inst_pid, ref cmd_id, cmd_pid, _, ref full) = all_commands[0];
                    let info = instances.iter().find(|i| i.pid == inst_pid).unwrap();
                    let url = instance_url(info, &None);
                    tracing::info!("Stopping only command: {} (PID {})", full, cmd_pid);
                    return stop_command_by_id(&client, &url, cmd_id, cmd_pid, inst_pid).await;
                }
                _ => {
                    tracing::warn!("Multiple commands running. Specify which one to stop:");
                    for (_, _, cmd_pid, _, full) in &all_commands {
                        tracing::warn!("  PID {} — {}", cmd_pid, full);
                    }
                    tracing::warn!("Usage: vrunner stop-command <PID or name>");
                    return Ok(false);
                }
            }
        }
    };

    // Fast path: if target is a pure number, treat as PID.
    if let Ok(pid) = target.parse::<u32>() {
        return handle_stop_command_by_pid_on_instances(&client, &instances, pid).await;
    }

    // Collect all commands from all instances.
    let all_commands = collect_all_commands(&client, &instances).await;

    if all_commands.is_empty() {
        return Ok(false);
    }

    // Round 1: exact match on name alone or full "name args" string.
    let exact: Vec<_> = all_commands.iter()
        .filter(|(_, _, _, name, full)| name == target || full == target)
        .collect();

    if exact.len() == 1 {
        let (inst_pid, ref cmd_id, cmd_pid, _, _) = exact[0];
        let info = instances.iter().find(|i| i.pid == *inst_pid).unwrap();
        let url = instance_url(info, &None);
        return stop_command_by_id(&client, &url, cmd_id, *cmd_pid, *inst_pid).await;
    }
    if exact.len() > 1 {
        tracing::warn!("Multiple commands match '{}':", target);
        for (inst_pid, _, cmd_pid, _, full) in &exact {
            tracing::warn!("  PID {} — {} (on instance {})", cmd_pid, full, inst_pid);
        }
        tracing::warn!("Use a PID to disambiguate.");
        return Ok(false);
    }

    // Round 2: prefix match on full "name args" string.
    let prefix_full: Vec<_> = all_commands.iter()
        .filter(|(_, _, _, _, full)| full.starts_with(target))
        .collect();

    if prefix_full.len() == 1 {
        let (inst_pid, ref cmd_id, cmd_pid, _, _) = prefix_full[0];
        let info = instances.iter().find(|i| i.pid == *inst_pid).unwrap();
        let url = instance_url(info, &None);
        return stop_command_by_id(&client, &url, cmd_id, *cmd_pid, *inst_pid).await;
    }
    if prefix_full.len() > 1 {
        tracing::warn!("Multiple commands match '{}':", target);
        for (inst_pid, _, cmd_pid, _, full) in &prefix_full {
            tracing::warn!("  PID {} — {} (on instance {})", cmd_pid, full, inst_pid);
        }
        tracing::warn!("Use a longer prefix or a PID to disambiguate.");
        return Ok(false);
    }

    // Round 3: prefix match on name alone.
    let prefix_name: Vec<_> = all_commands.iter()
        .filter(|(_, _, _, name, _)| name.starts_with(target))
        .collect();

    if prefix_name.len() == 1 {
        let (inst_pid, ref cmd_id, cmd_pid, _, _) = prefix_name[0];
        let info = instances.iter().find(|i| i.pid == *inst_pid).unwrap();
        let url = instance_url(info, &None);
        return stop_command_by_id(&client, &url, cmd_id, *cmd_pid, *inst_pid).await;
    }
    if prefix_name.len() > 1 {
        tracing::warn!("Multiple commands match '{}':", target);
        for (inst_pid, _, cmd_pid, _, full) in &prefix_name {
            tracing::warn!("  PID {} — {} (on instance {})", cmd_pid, full, inst_pid);
        }
        tracing::warn!("Use a longer prefix or a PID to disambiguate.");
        return Ok(false);
    }

    // No match at all.
    Ok(false)
}

/// Internal: send the kill request for a resolved command ID.
pub async fn stop_command_by_id(
    client: &reqwest::Client,
    url: &str,
    cmd_id: &str,
    cmd_pid: u32,
    inst_pid: u32,
) -> Result<bool> {
    let resp = client
        .post(format!("{}/api/commands/{}/kill", url, cmd_id))
        .json(&serde_json::json!({}))
        .send()
        .await;

    match resp {
        Ok(resp) => {
            let status = resp.status();
            let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::json!({"status": "unknown"}));
            if status.is_success() && body.get("status").and_then(|s| s.as_str()) == Some("ok") {
                println!("Command with PID {} stopped on instance {} (PID {})", cmd_pid, inst_pid, inst_pid);
                Ok(true)
            } else {
                let err_msg = body.get("error").and_then(|e| e.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| format!("HTTP {}", status));
                tracing::error!("Failed to stop command with PID {}: {}", cmd_pid, err_msg);
                Ok(false)
            }
        }
        Err(e) => {
            tracing::error!("Failed to stop command with PID {}: {}", cmd_pid, e);
            Ok(false)
        }
    }
}

/// Internal: stop a command by PID on a list of instances.
/// Used by handle_stop_command when target parses as a number.
pub async fn handle_stop_command_by_pid_on_instances(
    client: &reqwest::Client,
    instances: &[InstanceInfo],
    pid: u32,
) -> Result<bool> {
    for info in instances {
        let url = instance_url(info, &None);
        let cmd_id = match resolve_pid_to_id(client, &url, pid).await {
            Ok(id) => id,
            Err(_) => continue,
        };

        let resp = client
            .post(format!("{}/api/commands/{}/kill", url, cmd_id))
            .json(&serde_json::json!({}))
            .send()
            .await;

        match resp {
            Ok(resp) => {
                let status = resp.status();
                let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::json!({"status": "unknown"}));
                if status.is_success() && body.get("status").and_then(|s| s.as_str()) == Some("ok") {
                    println!("Command with PID {} stopped on instance {} (PID {})", pid, info.pid, info.pid);
                    return Ok(true);
                } else {
                    let err_msg = body.get("error").and_then(|e| e.as_str())
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| format!("HTTP {}", status));
                    tracing::error!("Failed to stop command with PID {}: {}", pid, err_msg);
                    return Ok(false);
                }
            }
            Err(e) => {
                tracing::error!("Failed to stop command with PID {}: {}", pid, e);
                return Ok(false);
            }
        }
    }

    Ok(false)
}
