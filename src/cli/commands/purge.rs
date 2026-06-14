#![cfg(feature = "vrw")]
#![allow(dead_code, unused_imports)]
use anyhow::Result;

use crate::cli::args::Cli;
use crate::cli::interactive_select::select_items;
use crate::instance::registry::InstanceRegistry;

use super::common::{
    build_command_select_items, collect_filtered_commands, http_client, instance_url,
    post_command_action_bool, resolve_target_command, SelectLabelStyle,
};

/// Purge a retained (exited) command by ID or name on any running instance.
///
/// Only exited commands (alive == false) are considered.
/// If no target is given and exactly one exited command exists, it is purged.
/// If the target matches a command ID (prefix), that command is purged.
/// Otherwise, matching proceeds by name (same strategy as stop).
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
    let all_commands = collect_filtered_commands(&client, &instances, |cmd| {
        !cmd.get("alive").and_then(|v| v.as_bool()).unwrap_or(true)
    })
    .await;

    // No target: purge the only exited command if there is exactly one,
    // or use interactive selection if -i was given.
    match target {
        None => match all_commands.len() {
            0 => {
                tracing::warn!("No exited commands to purge.");
                return Ok(false);
            }
            1 => {
                let (inst_pid, ref cmd_id, _, _, ref full) = all_commands[0];
                let info = instances.iter().find(|i| i.pid == inst_pid).unwrap();
                let url = instance_url(info, &None);
                tracing::info!("Purging only exited command: {} (ID {})", full, cmd_id);
                return purge_command_by_id(&client, &url, cmd_id, full).await;
            }
            _ => {
                if interactive {
                    let items = build_command_select_items(&all_commands, SelectLabelStyle::IdPrefixWithFull);
                    let selected = select_items(
                        &items,
                        "Select exited commands to purge [space-separated numbers]",
                    )?;
                    let mut purged_any = false;
                    for item in &selected {
                        if let Some((inst_pid, _, _, _, full)) =
                            all_commands.iter().find(|(_, id, _, _, _)| id == &item.id)
                        {
                            let info = instances.iter().find(|i| i.pid == *inst_pid).unwrap();
                            let url = instance_url(info, &None);
                            if purge_command_by_id(&client, &url, &item.id, full).await? {
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
        Some(t) => {
            // Fast path: if target matches a command ID prefix.
            let id_matches: Vec<_> = all_commands
                .iter()
                .filter(|(_, id, _, _, _)| id.starts_with(t))
                .collect();
            if id_matches.len() == 1 {
                let (inst_pid, ref cmd_id, _, _, ref full) = id_matches[0];
                let info = instances.iter().find(|i| i.pid == *inst_pid).unwrap();
                let url = instance_url(info, &None);
                return purge_command_by_id(&client, &url, cmd_id, full).await;
            }
            if id_matches.len() > 1 {
                tracing::warn!("Multiple exited commands match ID prefix '{}':", t);
                for (_, cmd_id, _, _, full) in &id_matches {
                    tracing::warn!("  {} — {}", cmd_id, full);
                }
                return Ok(false);
            }

            // Use standard target resolution (PID, name, prefix)
            let (inst_pid, ref cmd_id, _, _, ref full) =
                resolve_target_command(Some(t), &all_commands, "No exited command")?;
            let info = instances.iter().find(|i| i.pid == inst_pid).unwrap();
            let url = instance_url(info, &None);
            return purge_command_by_id(&client, &url, cmd_id, full).await;
        }
    }
}

/// Purge a specific command by ID via DELETE /api/commands/:id.
pub(crate) async fn purge_command_by_id(
    client: &reqwest::Client,
    url: &str,
    cmd_id: &str,
    label: &str,
) -> Result<bool> {
    post_command_action_bool(
        client,
        url,
        &format!("/api/commands/{}", cmd_id),
        None,
        reqwest::Method::DELETE,
        label,
        "purge",
        &format!("Purged: {}", label),
    )
    .await
}

#[cfg(test)]
mod tests {
    use clap::Parser;
    use super::*;
    use crate::cli::commands::common::http_client;

    #[tokio::test]
    async fn test_purge_command_by_id_connection_refused() {
        let client = http_client();
        // Connecting to a non-existent server should return Ok(false)
        let result =
            purge_command_by_id(&client, "http://127.0.0.1:1", "fake-id", "test-label").await;
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