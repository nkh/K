#![cfg(feature = "vrw")]

use anyhow::Result;

use crate::cli::args::Cli;
use crate::cli::commands::common::{http_client, resolve_instance, resolve_pid_to_id, VrwClient};
use crate::instance::registry::InstanceRegistry;

/// Handle the `vrw spawn` subcommand.
/// Discovers a running vrw instance and sends a spawn request via HTTP API.
pub async fn handle_spawn_command(
    cli: &Cli,
    cmd: &str,
    args: &[String],
    rows: Option<u16>,
    cols: Option<u16>,
    interactive: bool,
) -> Result<()> {
    let registry = InstanceRegistry::new()?;

    // When --interactive is set and multiple instances are running,
    // present an interactive picker so the user can choose by PID or port.
    let info = if interactive {
        let instances = registry.list_instances();
        if instances.is_empty() {
            anyhow::bail!("No running vrw instances found.");
        }
        if instances.len() == 1 {
            instances.into_iter().next().unwrap()
        } else {
            let items: Vec<_> = instances
                .iter()
                .map(|i| crate::cli::interactive_select::SelectItem {
                    label: format!("PID {} — port {}", i.pid, i.port),
                    id: i.pid.to_string(),
                })
                .collect();
            let selected = crate::cli::interactive_select::select_items(
                &items,
                "Select instance to spawn on [space-separated numbers]",
            )?;
            if selected.is_empty() {
                anyhow::bail!("No instance selected.");
            }
            let target_pid: u32 = selected[0].id.parse().unwrap();
            instances
                .into_iter()
                .find(|i| i.pid == target_pid)
                .expect("selected PID must exist")
        }
    } else {
        resolve_instance(cli, &registry)?
    };

    let client = http_client();
    let vrw = VrwClient::new(client, &info);

    let mut body = serde_json::json!({
        "cmd": cmd,
        "args": args,
    });

    // Add --env variables if provided
    let cli_env = cli.parse_env_vars();
    if !cli_env.is_empty() {
        body["env"] = serde_json::json!(cli_env);
    }

    // Add --no-env flag to skip config-level environment
    if cli.no_env {
        body["no_env"] = serde_json::json!(true);
    }

    // Add exit configuration if provided
    if let Some(ref on_exit) = cli.on_exit {
        body["on_exit"] = serde_json::json!(on_exit);
    }
    if let Some(ref on_error) = cli.on_error {
        body["on_error"] = serde_json::json!(on_error);
    }
    if let Some(timeout) = cli.exit_timeout {
        body["exit_timeout"] = serde_json::json!(timeout);
    }

    // Add profile if specified
    if let Some(ref profile) = cli.profile {
        body["profile"] = serde_json::json!(profile);
    }

    // Add per-command VTTY size if specified
    if let Some(r) = rows {
        body["rows"] = serde_json::json!(r);
    }
    if let Some(c_) = cols {
        body["cols"] = serde_json::json!(c_);
    }

    tracing::info!(
        target_pid = info.pid,
        cmd = cmd,
        url = %vrw.base_url(),
        "Spawning command on remote instance"
    );

    let data = vrw
        .post_data("/api/commands", &body)
        .await
        .map_err(|e| {
            tracing::error!(url = %vrw.base_url(), error = %e, "Failed to connect to vrw instance");
            anyhow::anyhow!(
                "Cannot connect to vrw instance at {} — is it running? Error: {}",
                vrw.base_url(),
                e
            )
        })?;

    let cmd_pid = data["pid"].as_u64().unwrap_or(0);
    let cmd_id = data["id"].as_str().unwrap_or("?");
    println!(
        "Command spawned successfully on instance {} (PID {})",
        info.pid, info.pid
    );
    println!("  PID:       {}", cmd_pid);
    println!(
        "  VTTY:      {}/api/commands/{}/vtty/html",
        vrw.base_url(),
        cmd_id
    );

    Ok(())
}

/// Shared implementation for freeze and thaw commands.
///
/// `signal` is the API path segment (`"freeze"` or `"thaw"`).
/// `signal_desc` is the human-readable description (`"SIGSTOP"` or `"SIGCONT"`).
async fn handle_signal_command(
    cli: &Cli,
    pid: Option<u32>,
    interactive: bool,
    signal: &str,
    signal_desc: &str,
) -> Result<()> {
    let registry = InstanceRegistry::new()?;
    let info = resolve_instance(cli, &registry)?;
    let client = http_client();
    let vrw = VrwClient::new(client, &info);

    if interactive {
        let all_commands =
            crate::cli::commands::common::collect_all_commands(
                vrw.client(),
                std::slice::from_ref(&info),
            )
            .await;
        let items = crate::cli::commands::common::build_command_select_items(
            &all_commands,
            crate::cli::commands::common::SelectLabelStyle::FullWithPid,
        );
        let selected = crate::cli::interactive_select::select_items(
            &items,
            &format!("Select commands to {} [space-separated numbers]", signal),
        )?;
        for item in &selected {
            if let Err(e) = vrw
                .post_action(&format!("/api/commands/{}/{}", item.id, signal), None)
                .await
            {
                tracing::error!("Failed to {} {}: {}", signal, item.id, e);
            } else {
                println!("Command {} {} ({})", item.id, signal, signal_desc);
            }
        }
        return Ok(());
    }

    let pid = match pid {
        Some(p) => p,
        None => {
            let all_commands =
                crate::cli::commands::common::collect_all_commands(
                    vrw.client(),
                    std::slice::from_ref(&info),
                )
                .await;
            if all_commands.len() == 1 {
                all_commands[0].2
            } else {
                anyhow::bail!(
                    "No PID specified and {} commands running. Use --interactive or specify a PID.",
                    all_commands.len()
                );
            }
        }
    };

    // Look up the command ID by PID via the instance's API
    let cmd_id = resolve_pid_to_id(&vrw, pid).await?;

    vrw.post_action(&format!("/api/commands/{}/{}", cmd_id, signal), None)
        .await
        .map_err(|e| {
            tracing::error!("Failed to {} command: {}", signal, e);
            anyhow::anyhow!("Failed to {} command: {}", signal, e)
        })?;

    println!(
        "Command with PID {} {} ({}) on instance {}",
        pid,
        signal,
        signal_desc,
        info.pid
    );

    Ok(())
}

/// Handle the `vrw freeze` subcommand.
pub async fn handle_freeze_command(cli: &Cli, pid: Option<u32>, interactive: bool) -> Result<()> {
    handle_signal_command(cli, pid, interactive, "freeze", "SIGSTOP").await
}

/// Handle the `vrw thaw` subcommand.
pub async fn handle_thaw_command(cli: &Cli, pid: Option<u32>, interactive: bool) -> Result<()> {
    handle_signal_command(cli, pid, interactive, "thaw", "SIGCONT").await
}

#[cfg(test)]
mod tests {
    use clap::Parser;
    use super::*;

    #[tokio::test]
    async fn test_handle_spawn_command_no_instances() {
        let cli = crate::cli::args::Cli::try_parse_from(["vrw", "spawn", "htop"]).unwrap();
        let result = handle_spawn_command(&cli, "htop", &[], None, None, false).await;
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("No running vrw instances"), "unexpected: {}", msg);
    }

    #[tokio::test]
    async fn test_handle_freeze_command_no_instances() {
        let cli = crate::cli::args::Cli::try_parse_from(["vrw", "freeze"]).unwrap();
        let result = handle_freeze_command(&cli, None, false).await;
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("No running vrw instances"), "unexpected: {}", msg);
    }

    #[tokio::test]
    async fn test_handle_thaw_command_no_instances() {
        let cli = crate::cli::args::Cli::try_parse_from(["vrw", "thaw"]).unwrap();
        let result = handle_thaw_command(&cli, None, false).await;
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("No running vrw instances"), "unexpected: {}", msg);
    }
}
