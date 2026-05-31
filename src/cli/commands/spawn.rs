#![cfg(feature = "vrunner")]
#![allow(dead_code, unused_imports)]
use anyhow::Result;

use crate::cli::args::Cli;
use crate::instance::registry::InstanceRegistry;

use super::common::{http_client, instance_url, resolve_instance, resolve_pid_to_id};

/// Handle the `vrunner spawn` subcommand.
/// Discovers a running vrunner instance and sends a spawn request via HTTP API.
pub async fn handle_spawn_command(
    cli: &Cli,
    cmd: &str,
    args: &[String],
    rows: Option<u16>,
    cols: Option<u16>,
) -> Result<()> {
    let registry = InstanceRegistry::new()?;
    let info = resolve_instance(cli, &registry)?;

    let url = instance_url(&info, &None);
    let client = http_client();

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

    tracing::info!(target_pid = info.pid, cmd = cmd, url = %url, "Spawning command on remote instance");

    let resp = client
        .post(format!("{}/api/commands", url))
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            tracing::error!(url = %url, error = %e, "Failed to connect to vrunner instance");
            anyhow::anyhow!(
                "Cannot connect to vrunner instance at {} — is it running? Error: {}",
                url,
                e
            )
        })?;

    let status = resp.status();
    let result: serde_json::Value = resp.json().await?;

    if status.is_success() {
        let cmd_pid = result["data"]["pid"].as_u64().unwrap_or(0);
        let cmd_id = result["data"]["id"].as_str().unwrap_or("?");
        println!(
            "Command spawned successfully on instance {} (PID {})",
            info.pid, info.pid
        );
        println!("  PID:       {}", cmd_pid);
        println!("  VTTY:      {}/api/commands/{}/vtty/html", url, cmd_id);
    } else {
        let error = result["error"].as_str().unwrap_or("Unknown error");
        tracing::error!("Failed to spawn command: {}", error);
        std::process::exit(1);
    }

    Ok(())
}

/// Handle the `vrunner freeze` subcommand.
pub async fn handle_freeze_command(cli: &Cli, pid: u32) -> Result<()> {
    let registry = InstanceRegistry::new()?;
    let info = resolve_instance(cli, &registry)?;
    let url = instance_url(&info, &None);
    let client = http_client();

    // Look up the command ID by PID via the instance's API
    let cmd_id = resolve_pid_to_id(&client, &url, pid).await?;

    let resp = client
        .post(format!("{}/api/commands/{}/freeze", url, cmd_id))
        .send()
        .await?;

    let status = resp.status();
    let result: serde_json::Value = resp.json().await?;

    if status.is_success() {
        println!(
            "Command with PID {} frozen (SIGSTOP) on instance {}",
            pid, info.pid
        );
    } else {
        let error = result["error"].as_str().unwrap_or("Unknown error");
        tracing::error!("Failed to freeze command: {}", error);
        std::process::exit(1);
    }

    Ok(())
}

/// Handle the `vrunner thaw` subcommand.
pub async fn handle_thaw_command(cli: &Cli, pid: u32) -> Result<()> {
    let registry = InstanceRegistry::new()?;
    let info = resolve_instance(cli, &registry)?;
    let url = instance_url(&info, &None);
    let client = http_client();

    // Look up the command ID by PID via the instance's API
    let cmd_id = resolve_pid_to_id(&client, &url, pid).await?;

    let resp = client
        .post(format!("{}/api/commands/{}/thaw", url, cmd_id))
        .send()
        .await?;

    let status = resp.status();
    let result: serde_json::Value = resp.json().await?;

    if status.is_success() {
        println!(
            "Command with PID {} thawed (SIGCONT) on instance {}",
            pid, info.pid
        );
    } else {
        let error = result["error"].as_str().unwrap_or("Unknown error");
        tracing::error!("Failed to thaw command: {}", error);
        std::process::exit(1);
    }

    Ok(())
}
