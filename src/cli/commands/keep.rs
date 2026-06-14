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
    let all_commands = collect_filtered_commands(&client, &instances, |cmd| {
        cmd.get("alive").and_then(|v| v.as_bool()).unwrap_or(false)
    })
    .await;

    // When target is specified, try resolve_target_command (handles PID, name, prefix).
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
        let items = build_command_select_items(&all_commands, SelectLabelStyle::IdPrefixWithFull);
        if items.is_empty() {
            tracing::warn!("No running commands to keep.");
            return Ok(false);
        }
        let selected = select_items(&items, "Select commands to keep [space-separated numbers]")?;
        let mut kept_any = false;
        for item in &selected {
            if let Some((inst_pid, _, _, _, full)) = all_commands.iter().find(|(_, id, _, _, _)| id == &item.id) {
                let info = instances.iter().find(|i| i.pid == *inst_pid).unwrap();
                let url = instance_url(info, &None);
                if keep_command_by_id(&client, &url, &item.id, full).await? {
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
    let all_commands = collect_filtered_commands(&client, &instances, |cmd| {
        cmd.get("exit")
            .and_then(|e| e.get("retain_on_exit"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    })
    .await;

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
        let items = build_command_select_items(&all_commands, SelectLabelStyle::IdPrefixWithFull);
        if items.is_empty() {
            tracing::warn!("No kept commands to unkeep.");
            return Ok(false);
        }
        let selected = select_items(&items, "Select commands to unkeep [space-separated numbers]")?;
        let mut unkept_any = false;
        for item in &selected {
            if let Some((inst_pid, _, _, _, full)) = all_commands.iter().find(|(_, id, _, _, _)| id == &item.id) {
                let info = instances.iter().find(|i| i.pid == *inst_pid).unwrap();
                let url = instance_url(info, &None);
                if unkeep_command_by_id(&client, &url, &item.id, full).await? {
                    unkept_any = true;
                }
            }
        }
        return Ok(unkept_any);
    }

    Ok(false)
}

async fn keep_command_by_id(
    client: &reqwest::Client,
    url: &str,
    cmd_id: &str,
    label: &str,
) -> Result<bool> {
    post_command_action_bool(
        client,
        url,
        &format!("/api/commands/{}/keep", cmd_id),
        None,
        reqwest::Method::POST,
        label,
        "keep",
        &format!("Kept: {} (terminal rendering retained after exit)", label),
    )
    .await
}

async fn unkeep_command_by_id(
    client: &reqwest::Client,
    url: &str,
    cmd_id: &str,
    label: &str,
) -> Result<bool> {
    post_command_action_bool(
        client,
        url,
        &format!("/api/commands/{}/unkeep", cmd_id),
        None,
        reqwest::Method::POST,
        label,
        "unkeep",
        &format!("Unkept: {} (will be removed on exit)", label),
    )
    .await
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