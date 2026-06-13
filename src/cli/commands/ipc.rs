//! CLI subcommand handlers for inter-instance IPC.
//!
//! These commands connect to a running vrc instance via its UDS control
//! socket and send commands (keys, cat, spawn, freeze, thaw, resize, etc.).

use anyhow::Result;
use std::io::{stdout, Write};

use crate::instance::registry::InstanceRegistry;
use crate::ipc::client::send_command;
use crate::ipc::protocol::{ControlCommand, ControlResponse};

/// Resolve a command ID from an optional `-c` flag or fall back to the
/// first command returned by `list` from the target instance.
pub(crate) async fn resolve_command_id(pid: u32, command: Option<&str>) -> Result<String> {
    match command {
        Some(id) => Ok(id.to_string()),
        None => {
            // Query the instance for its command list and pick the first
            let response = send_command(pid, ControlCommand::List).await?;
            match response {
                ControlResponse::Ok { data } => {
                    let commands = data.get("commands").and_then(|v| v.as_array());
                    match commands.and_then(|cmds| cmds.first()) {
                        Some(first) => {
                            let id = first.get("id").and_then(|v| v.as_str()).unwrap_or("");
                            if id.is_empty() {
                                anyhow::bail!("No commands running in instance {}", pid);
                            }
                            Ok(id.to_string())
                        }
                        None => anyhow::bail!("No commands running in instance {}", pid),
                    }
                }
                ControlResponse::Error { error } => anyhow::bail!("{}", error),
            }
        }
    }
}

/// Handle `vrc keys <pid> <keys>`.
pub async fn handle_keys_command(pid: u32, command: Option<&str>, keys: &str) -> Result<()> {
    let id = resolve_command_id(pid, command).await?;
    let response = send_command(pid, ControlCommand::SendKeys {
        id,
        keys: keys.to_string(),
    })
    .await?;
    match response {
        ControlResponse::Ok { .. } => {
            println!("Keystrokes sent.");
        }
        ControlResponse::Error { error } => anyhow::bail!("{}", error),
    }
    Ok(())
}

/// Handle `vrc cat <pid>`.
pub async fn handle_cat_command(pid: u32, command: Option<&str>) -> Result<()> {
    let id = resolve_command_id(pid, command).await?;
    let response = send_command(pid, ControlCommand::Cat { id }).await?;
    match response {
        ControlResponse::Ok { data } => {
            let text = data.get("text").and_then(|v| v.as_str()).unwrap_or("");
            print!("{}", text);
            stdout().flush()?;
        }
        ControlResponse::Error { error } => anyhow::bail!("{}", error),
    }
    Ok(())
}

/// Handle `vrc spawn-in <pid> -- cmd args...`.
pub async fn handle_spawn_in_command(pid: u32, cmd: &str, args: &[String]) -> Result<()> {
    let response = send_command(
        pid,
        ControlCommand::Spawn {
            cmd: cmd.to_string(),
            args: args.to_vec(),
            env: None,
            rows: None,
            cols: None,
            dir: None,
        },
    )
    .await?;
    match response {
        ControlResponse::Ok { data } => {
            let id = data.get("id").and_then(|v| v.as_str()).unwrap_or("?");
            println!("Spawned command {} in instance {}", id, pid);
        }
        ControlResponse::Error { error } => anyhow::bail!("{}", error),
    }
    Ok(())
}

/// Handle `vrc freeze <pid>`.
pub async fn handle_freeze_command(pid: u32, command: Option<&str>) -> Result<()> {
    let id = resolve_command_id(pid, command).await?;
    let response = send_command(pid, ControlCommand::Freeze { id }).await?;
    match response {
        ControlResponse::Ok { .. } => {
            println!("Command frozen.");
        }
        ControlResponse::Error { error } => anyhow::bail!("{}", error),
    }
    Ok(())
}

/// Handle `vrc thaw <pid>`.
pub async fn handle_thaw_command(pid: u32, command: Option<&str>) -> Result<()> {
    let id = resolve_command_id(pid, command).await?;
    let response = send_command(pid, ControlCommand::Thaw { id }).await?;
    match response {
        ControlResponse::Ok { .. } => {
            println!("Command thawed.");
        }
        ControlResponse::Error { error } => anyhow::bail!("{}", error),
    }
    Ok(())
}

/// Handle `vrc resize <pid> --rows N --cols M`.
pub async fn handle_resize_command(
    pid: u32,
    command: Option<&str>,
    rows: u16,
    cols: u16,
) -> Result<()> {
    let id = resolve_command_id(pid, command).await?;
    let response = send_command(pid, ControlCommand::Resize { id, rows, cols }).await?;
    match response {
        ControlResponse::Ok { .. } => {
            println!("VTTY resized to {}x{}.", rows, cols);
        }
        ControlResponse::Error { error } => anyhow::bail!("{}", error),
    }
    Ok(())
}

/// Handle `vrc kill <pid>` — kill a command in a running instance.
pub async fn handle_kill_command(pid: u32, command: Option<&str>) -> Result<()> {
    let id = resolve_command_id(pid, command).await?;
    let response = send_command(pid, ControlCommand::Kill { id: id.clone() }).await?;
    match response {
        ControlResponse::Ok { .. } => {
            println!("Command {} killed.", id);
        }
        ControlResponse::Error { error } => anyhow::bail!("{}", error),
    }
    Ok(())
}

/// Kill all commands in a running vrc instance.
pub async fn handle_kill_all_commands(pid: u32) -> Result<()> {
    let response = send_command(pid, ControlCommand::List).await?;
    let commands = match response {
        ControlResponse::Ok { data } => data
            .get("commands")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default(),
        ControlResponse::Error { error } => anyhow::bail!("{}", error),
    };

    if commands.is_empty() {
        println!("No commands running in instance {}.", pid);
        return Ok(());
    }

    let mut killed = 0u32;
    for cmd in &commands {
        let id = cmd.get("id").and_then(|v| v.as_str()).unwrap_or("");
        if !id.is_empty() {
            let response = send_command(pid, ControlCommand::Kill { id: id.to_string() }).await?;
            match response {
                ControlResponse::Ok { .. } => { killed += 1; }
                ControlResponse::Error { error } => {
                    tracing::error!("Failed to kill command {}: {}", id, error);
                }
            }
        }
    }
    println!("Killed {} command(s) in instance {}.", killed, pid);
    Ok(())
}

/// Verify a target PID is a live vrc instance.
/// Returns an error if not.
pub fn verify_instance(pid: u32) -> Result<()> {
    let registry = InstanceRegistry::new()?;
    let instances = registry.list_instances();
    if !instances.iter().any(|i| i.pid == pid) {
        let available: Vec<String> = instances.iter().map(|i| i.pid.to_string()).collect();
        if available.is_empty() {
            anyhow::bail!("No running vrc instances found.");
        } else {
            anyhow::bail!(
                "No vrc instance with PID {}. Running instances: {}",
                pid,
                available.join(", ")
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_command_id_explicit_command() {


