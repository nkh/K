use anyhow::Result;

use crate::cli::args::Cli;
use crate::instance::info::InstanceInfo;
use crate::instance::registry::InstanceRegistry;
use crate::interactive::display::detect_terminal_size;

use super::common::{collect_all_commands, http_client, instance_url, resolve_pid_to_id};

/// Handle the `vrunner resize-command` subcommand.
///
/// Resizes the VTTY of a running command by PID or name.
/// Resizes both the in-memory buffer and the child PTY (sends SIGWINCH).
/// If rows/cols are 0 (default), uses the current terminal size.
pub async fn handle_resize_command(_cli: &Cli, target: &str, rows: u16, cols: u16) -> Result<()> {
    let registry = InstanceRegistry::new()?;
    let instances = registry.list_instances();

    if instances.is_empty() {
        anyhow::bail!(
            "No running vrunner instances found. Start one first with: vrunner -- <command>"
        );
    }

    // If rows/cols are 0 (default), detect from the current terminal.
    let (rows, cols) = if rows == 0 || cols == 0 {
        match detect_terminal_size() {
            Some((r, c)) => {
                let r = if rows == 0 { r } else { rows };
                let c = if cols == 0 { c } else { cols };
                (r, c)
            }
            None => {
                let r = if rows == 0 { 24 } else { rows };
                let c = if cols == 0 { 80 } else { cols };
                (r, c)
            }
        }
    } else {
        (rows, cols)
    };

    let client = http_client();

    // Fast path: if target is a pure number, treat as PID.
    if let Ok(pid) = target.parse::<u32>() {
        return handle_resize_by_pid(&client, &instances, pid, rows, cols).await;
    }

    // Collect all commands from all instances.
    let all_commands = collect_all_commands(&client, &instances).await;

    if all_commands.is_empty() {
        anyhow::bail!("No running commands found. Use `vrunner list` to see running commands.");
    }

    // Exact match on name alone or full "name args" string.
    let exact: Vec<_> = all_commands
        .iter()
        .filter(|(_, _, _, name, full)| name == target || full == target)
        .collect();

    if exact.len() == 1 {
        let (inst_pid, ref cmd_id, cmd_pid, _, _) = exact[0];
        let info = instances.iter().find(|i| i.pid == *inst_pid).unwrap();
        let url = instance_url(info, &None);
        return resize_command_by_id(&client, &url, cmd_id, *cmd_pid, *inst_pid, rows, cols).await;
    }
    if exact.len() > 1 {
        tracing::warn!("Multiple commands match '{}':", target);
        for (_, _, pid, _name, full) in &exact {
            tracing::warn!("  PID {} — {}", pid, full);
        }
        anyhow::bail!("Ambiguous target. Use PID to disambiguate.");
    }

    // Prefix match on full string, then on name only (same as stop-command).
    let prefix_full: Vec<_> = all_commands
        .iter()
        .filter(|(_, _, _, _, full)| full.starts_with(target))
        .collect();
    if prefix_full.len() == 1 {
        let (inst_pid, ref cmd_id, cmd_pid, _, _) = prefix_full[0];
        let info = instances.iter().find(|i| i.pid == *inst_pid).unwrap();
        let url = instance_url(info, &None);
        return resize_command_by_id(&client, &url, cmd_id, *cmd_pid, *inst_pid, rows, cols).await;
    }

    let prefix_name: Vec<_> = all_commands
        .iter()
        .filter(|(_, _, _, name, _)| name.starts_with(target))
        .collect();
    if prefix_name.len() == 1 {
        let (inst_pid, ref cmd_id, cmd_pid, _, _) = prefix_name[0];
        let info = instances.iter().find(|i| i.pid == *inst_pid).unwrap();
        let url = instance_url(info, &None);
        return resize_command_by_id(&client, &url, cmd_id, *cmd_pid, *inst_pid, rows, cols).await;
    }

    anyhow::bail!(
        "No command matching '{}' found. Use `vrunner list` to see running commands.",
        target
    );
}

/// Resize a command by its UUID via the instance's HTTP API.
pub async fn resize_command_by_id(
    client: &reqwest::Client,
    url: &str,
    cmd_id: &str,
    cmd_pid: u32,
    inst_pid: u32,
    rows: u16,
    cols: u16,
) -> Result<()> {
    let resp = client
        .post(format!("{}/api/commands/{}/resize", url, cmd_id))
        .json(&serde_json::json!({ "rows": rows, "cols": cols }))
        .send()
        .await?;

    let status = resp.status();
    let body: serde_json::Value = resp.json().await?;

    if status.is_success() && body.get("status").and_then(|s| s.as_str()) == Some("ok") {
        println!(
            "Resized command with PID {} to {}x{} on instance {} (PID {})",
            cmd_pid, rows, cols, inst_pid, inst_pid
        );
        Ok(())
    } else {
        let err_msg = body
            .get("error")
            .and_then(|e| e.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("HTTP {}", status));
        anyhow::bail!("Failed to resize command with PID {}: {}", cmd_pid, err_msg);
    }
}

/// Resize a command by its OS PID, trying all running instances.
pub async fn handle_resize_by_pid(
    client: &reqwest::Client,
    instances: &[InstanceInfo],
    pid: u32,
    rows: u16,
    cols: u16,
) -> Result<()> {
    for info in instances {
        let url = instance_url(info, &None);
        match resolve_pid_to_id(client, &url, pid).await {
            Ok(cmd_id) => {
                return resize_command_by_id(client, &url, &cmd_id, pid, info.pid, rows, cols)
                    .await;
            }
            Err(_) => continue,
        }
    }
    anyhow::bail!(
        "No command found with PID {}. Use `vrunner list` to see running commands.",
        pid
    );
}
