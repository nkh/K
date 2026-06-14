#![cfg(feature = "vrw")]

use anyhow::Result;

use crate::cli::args::Cli;
use crate::cli::interactive_select::select_items;
use crate::instance::registry::InstanceRegistry;

use super::common::{
    build_command_select_items, collect_filtered_commands, http_client,
    resolve_target_command, VrwClient, SelectLabelStyle,
};

/// Tag or untag a running command's retain-on-exit flag.
///
/// This is a generic handler used by both `keep` and `unkeep` commands.
/// The only differences are: the filter predicate, the API endpoint,
/// the interactive prompt message, and the success/error messages.
async fn handle_toggle_command(
    _cli: &Cli,
    target: Option<&str>,
    interactive: bool,
    filter: fn(&serde_json::Value) -> bool,
    endpoint: &'static str,
    select_prompt: &str,
    empty_warn: &str,
    not_found_msg: &str,
    verb: &str,
    success_fmt: fn(&str) -> String,
) -> Result<bool> {
    let registry = InstanceRegistry::new()?;
    let instances = registry.list_instances();

    if instances.is_empty() {
        return Ok(false);
    }

    let client = http_client();
    let all_commands = collect_filtered_commands(&client, &instances, filter).await;

    let resolved = if target.is_some() {
        resolve_target_command(target, &all_commands, not_found_msg).ok()
    } else if all_commands.len() == 1 {
        Some(all_commands[0].clone())
    } else {
        None
    };

    if let Some((inst_pid, cmd_id, _cmd_pid, _, full)) = resolved {
        let info = instances.iter().find(|i| i.pid == inst_pid).unwrap();
        let vrw = VrwClient::new(http_client(), info);
        let msg = success_fmt(&full);
        return vrw.post_action_quiet(
            &format!("/api/commands/{}/{}", cmd_id, endpoint),
            None,
            reqwest::Method::POST,
            &full,
            verb,
            &msg,
        ).await;
    }

    if interactive {
        let items = build_command_select_items(&all_commands, SelectLabelStyle::IdPrefixWithFull);
        if items.is_empty() {
            tracing::warn!("{}", empty_warn);
            return Ok(false);
        }
        let selected = select_items(&items, select_prompt)?;
        let mut acted_any = false;
        for item in &selected {
            if let Some((inst_pid, _, _, _, full)) = all_commands.iter().find(|(_, id, _, _, _)| id == &item.id) {
                let info = instances.iter().find(|i| i.pid == *inst_pid).unwrap();
                let vrw = VrwClient::new(http_client(), info);
                let msg = success_fmt(full);
                if vrw.post_action_quiet(
                    &format!("/api/commands/{}/{}", item.id, endpoint),
                    None,
                    reqwest::Method::POST,
                    full,
                    verb,
                    &msg,
                ).await? {
                    acted_any = true;
                }
            }
        }
        return Ok(acted_any);
    }

    Ok(false)
}

/// Tag a running command to retain its VTTY buffer after exit.
pub async fn handle_keep_command(cli: &Cli, target: Option<&str>, interactive: bool) -> Result<bool> {
    handle_toggle_command(
        cli, target, interactive,
        |cmd| cmd.get("alive").and_then(|v| v.as_bool()).unwrap_or(false),
        "keep",
        "Select commands to keep [space-separated numbers]",
        "No running commands to keep.",
        "No running command",
        "keep",
        |full| format!("Kept: {} (terminal rendering retained after exit)", full),
    ).await
}

/// Unkeep: remove retain_on_exit tag from commands.
pub async fn handle_unkeep_command(cli: &Cli, target: Option<&str>, interactive: bool) -> Result<bool> {
    handle_toggle_command(
        cli, target, interactive,
        |cmd| cmd.get("exit")
            .and_then(|e| e.get("retain_on_exit"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        "unkeep",
        "Select commands to unkeep [space-separated numbers]",
        "No kept commands to unkeep.",
        "No kept command",
        "unkeep",
        |full| format!("Unkept: {} (will be removed on exit)", full),
    ).await
}

#[cfg(test)]
mod tests {
    use clap::Parser;
    use super::*;
    use crate::cli::commands::common::http_client;

    fn make_test_instance() -> crate::instance::info::InstanceInfo {
        crate::instance::info::InstanceInfo {
            pid: 1, port: 1, bind: "127.0.0.1".to_string(),
            name: None, start_time: chrono::Utc::now(),
            daemon: false, display: false, command: None,
        }
    }

    #[tokio::test]
    async fn test_keep_command_by_id_connection_refused() {
        let info = make_test_instance();
        let vrw = VrwClient::new(http_client(), &info);
        let result = vrw.post_action_quiet(
            "/api/commands/fake-id/keep", None,
            reqwest::Method::POST, "test", "keep",
            "Kept: test (terminal rendering retained after exit)",
        ).await;
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[tokio::test]
    async fn test_unkeep_command_by_id_connection_refused() {
        let info = make_test_instance();
        let vrw = VrwClient::new(http_client(), &info);
        let result = vrw.post_action_quiet(
            "/api/commands/fake-id/unkeep", None,
            reqwest::Method::POST, "test", "unkeep",
            "Unkept: test (will be removed on exit)",
        ).await;
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