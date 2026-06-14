#![cfg(feature = "vrw")]
#![allow(dead_code, unused_imports)]
use anyhow::Result;

use crate::cli::args::Cli;
use crate::cli::commands::common::{
    build_command_select_items, collect_all_commands, http_client, resolve_pid_to_id,
    resolve_target_command, SelectLabelStyle, VrwClient,
};
use crate::instance::registry::InstanceRegistry;
use crate::interactive::display::detect_terminal_size;

/// Handle the `vrw resize-command` subcommand.
///
/// Resizes the VTTY of a running command by PID or name.
/// Resizes both the in-memory buffer and the child PTY (sends SIGWINCH).
/// If rows/cols are 0 (default), uses the current terminal size.
pub async fn handle_resize_command(
    _cli: &Cli,
    target: Option<&str>,
    rows: u16,
    cols: u16,
    interactive: bool,
) -> Result<()> {
    let registry = InstanceRegistry::new()?;
    let instances = registry.list_instances();

    if instances.is_empty() {
        anyhow::bail!(
            "No running vrw instances found. Start one first with: vrw -- <command>"
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

    // Interactive mode: list all commands and let user select
    if interactive && target.is_none() {
        let all_commands = collect_all_commands(&client, &instances).await;
        if all_commands.is_empty() {
            anyhow::bail!("No running commands found. Use `vrw list` to see running commands.");
        }
        let items = build_command_select_items(&all_commands, SelectLabelStyle::FullWithPid);
        let selected = crate::cli::interactive_select::select_items(
            &items,
            "Select commands to resize [space-separated numbers]",
        )?;
        for item in &selected {
            let all_cmds = collect_all_commands(&client, &instances).await;
            if let Some((inst_pid, cmd_id, cmd_pid, _, _)) =
                all_cmds.iter().find(|(_, id, _, _, _)| id == &item.id)
            {
                let info = instances.iter().find(|i| i.pid == *inst_pid).unwrap();
                let vrw = VrwClient::new(client.clone(), info);
                if let Err(e) = resize_command_by_id(&vrw, cmd_id, *cmd_pid, rows, cols).await {
                    tracing::error!("Failed to resize command {}: {}", cmd_pid, e);
                }
            }
        }
        return Ok(());
    }

    // If target is None, error.
    let target = match target {
        Some(t) => t,
        None => anyhow::bail!(
            "No target specified. Use `vrw resize <PID or name>` or `vrw resize --interactive`."
        ),
    };

    // Fast path: if target is a pure number, treat as PID.
    if let Ok(pid) = target.parse::<u32>() {
        return handle_resize_by_pid(&client, &instances, pid, rows, cols).await;
    }

    let all_commands = collect_all_commands(&client, &instances).await;

    if all_commands.is_empty() {
        anyhow::bail!("No running commands found. Use `vrw list` to see running commands.");
    }

    let (inst_pid, cmd_id, cmd_pid, _, _) =
        resolve_target_command(Some(target), &all_commands, "No command")?;
    let info = instances.iter().find(|i| i.pid == inst_pid).unwrap();
    let vrw = VrwClient::new(client, info);
    resize_command_by_id(&vrw, &cmd_id, cmd_pid, rows, cols).await
}

/// Resize a command by its UUID via a [`VrwClient`].
async fn resize_command_by_id(
    client: &VrwClient,
    cmd_id: &str,
    cmd_pid: u32,
    rows: u16,
    cols: u16,
) -> Result<()> {
    client
        .post_action(
            &format!("/api/commands/{}/resize", cmd_id),
            Some(&serde_json::json!({ "rows": rows, "cols": cols })),
        )
        .await?;
    println!(
        "Resized command with PID {} to {}x{} on instance {}",
        cmd_pid,
        rows,
        cols,
        client.instance_pid()
    );
    Ok(())
}

/// Resize a command by its OS PID, trying all running instances.
pub async fn handle_resize_by_pid(
    client: &reqwest::Client,
    instances: &[crate::instance::info::InstanceInfo],
    pid: u32,
    rows: u16,
    cols: u16,
) -> Result<()> {
    for info in instances {
        let vrw = VrwClient::new(client.clone(), info);
        match resolve_pid_to_id(client, vrw.base_url(), pid).await {
            Ok(cmd_id) => {
                return resize_command_by_id(&vrw, &cmd_id, pid, rows, cols).await;
            }
            Err(_) => continue,
        }
    }
    anyhow::bail!(
        "No command found with PID {}. Use `vrw list` to see running commands.",
        pid
    );
}

#[cfg(test)]
mod tests {
    use clap::Parser;
    use super::*;
    use crate::cli::commands::common::http_client;

    #[tokio::test]
    async fn test_resize_command_by_id_connection_refused() {
        let client = http_client();
        let info = crate::instance::info::InstanceInfo {
            pid: 999,
            port: 1,
            bind: "127.0.0.1".to_string(),
            name: None,
            start_time: chrono::Utc::now(),
            daemon: false,
            display: false,
            command: None,
        };
        let vrw = VrwClient::new(client, &info);
        let result = resize_command_by_id(&vrw, "fake-id", 100, 50, 80).await;
        assert!(result.is_err(), "connection refused should error");
    }

    #[tokio::test]
    async fn test_handle_resize_command_no_instances() {
        let cli = crate::cli::args::Cli::try_parse_from(["vrw", "resize"]).unwrap();
        let result = handle_resize_command(&cli, None, 0, 0, false).await;
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("No running vrw instances"), "unexpected error: {}", msg);
    }

    #[tokio::test]
    async fn test_handle_resize_by_pid_no_match() {
        let client = http_client();
        let instances: Vec<crate::instance::info::InstanceInfo> = vec![];
        let result = handle_resize_by_pid(&client, &instances, 9999, 24, 80).await;
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("No command found with PID 9999"),
            "unexpected: {}",
            msg
        );
    }
}
