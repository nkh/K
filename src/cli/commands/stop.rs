use anyhow::Result;

use crate::instance::info::InstanceInfo;

// ── vrl (signal-based) implementation ──

/// Stop a vrl instance by sending SIGTERM to its PID.
#[cfg(not(feature = "vrunner"))]
pub fn handle_stop_command(pid: Option<u32>, instances: &[InstanceInfo]) -> Result<()> {
    let target_pid = resolve_stop_target(pid, instances);

    #[cfg(unix)]
    {
        let ret = unsafe { libc::kill(target_pid as i32, libc::SIGTERM) };
        if ret != 0 {
            let errno = std::io::Error::last_os_error();
            anyhow::bail!(
                "Failed to stop instance {} (PID {}): {}",
                target_pid, target_pid, errno
            );
        }
        println!("Sent SIGTERM to vrl instance (PID {})", target_pid);
    }
    #[cfg(not(unix))]
    {
        std::process::Command::new("kill")
            .arg(target_pid.to_string())
            .spawn()?
            .wait()?;
        println!("Sent kill signal to vrl instance (PID {})", target_pid);
    }

    Ok(())
}

/// Resolve the target PID for the `stop` subcommand.
///
/// Shared core logic: when `pid` is `None`, auto-selects the sole instance or
/// errors with a disambiguation list.  The `binary_name` and `extra_info`
/// closure allow each binary to customise its messages.
#[cfg(not(feature = "vrunner"))]
pub fn resolve_stop_target(pid: Option<u32>, instances: &[InstanceInfo]) -> u32 {
    resolve_stop_target_impl(pid, instances, "vrl", |_| String::new())
}

#[cfg(feature = "vrunner")]
pub fn resolve_stop_target(pid: Option<u32>, instances: &[InstanceInfo]) -> u32 {
    resolve_stop_target_impl(pid, instances, "vrunner", |inst| {
        format!(" — port {}", inst.port)
    })
}

/// Common implementation for resolving which instance to stop.
fn resolve_stop_target_impl(
    pid: Option<u32>,
    instances: &[InstanceInfo],
    binary_name: &str,
    extra_info: impl Fn(&InstanceInfo) -> String,
) -> u32 {
    match pid {
        Some(p) => p,
        None => match instances.len() {
            0 => {
                eprintln!("No {} instances running.", binary_name);
                std::process::exit(1);
            }
            1 => {
                let p = instances[0].pid;
                println!("Stopping only running instance (PID {})", p);
                p
            }
            _ => {
                eprintln!("Multiple {} instances running. Specify which one to stop:", binary_name);
                for inst in instances {
                    eprintln!("  PID {}{}", inst.pid, extra_info(inst));
                }
                eprintln!("Usage: {} stop <PID>", binary_name);
                std::process::exit(1);
            }
        },
    }
}

// ── vrunner (HTTP-based) implementation ──

#[cfg(feature = "vrunner")]
use super::common::{collect_all_commands, http_client, instance_url, resolve_pid_to_id};

/// Stop a specific command by PID or name on any running instance (vrunner).
#[cfg(feature = "vrunner")]
pub async fn handle_stop_command(_cli: &crate::cli::args::Cli, target: Option<&str>) -> Result<bool> {
    let registry = crate::instance::registry::InstanceRegistry::new()?;
    let instances = registry.list_instances();

    if instances.is_empty() {
        return Ok(false);
    }

    let client = http_client();

    let target = match target {
        Some(t) => t,
        None => {
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

    if let Ok(pid) = target.parse::<u32>() {
        return handle_stop_command_by_pid_on_instances(&client, &instances, pid).await;
    }

    let all_commands = collect_all_commands(&client, &instances).await;
    if all_commands.is_empty() {
        return Ok(false);
    }

    // Try exact match on name or full string
    let exact: Vec<_> = all_commands
        .iter()
        .filter(|(_, _, _, name, full)| name == target || full == target)
        .collect();

    if exact.len() == 1 {
        let (inst_pid, ref cmd_id, cmd_pid, _, _) = exact[0];
        let info = instances.iter().find(|i| i.pid == *inst_pid).unwrap();
        let url = instance_url(info, &None);
        return stop_command_by_id(&client, &url, cmd_id, *cmd_pid, *inst_pid).await;
    }

    Ok(false)
}

#[cfg(feature = "vrunner")]
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
            let body: serde_json::Value = resp
                .json()
                .await
                .unwrap_or(serde_json::json!({"status": "unknown"}));
            if status.is_success() && body.get("status").and_then(|s| s.as_str()) == Some("ok") {
                println!(
                    "Command with PID {} stopped on instance {} (PID {})",
                    cmd_pid, inst_pid, inst_pid
                );
                Ok(true)
            } else {
                let err_msg = body
                    .get("error")
                    .and_then(|e| e.as_str())
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

#[cfg(feature = "vrunner")]
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
                let body: serde_json::Value = resp
                    .json()
                    .await
                    .unwrap_or(serde_json::json!({"status": "unknown"}));
                if status.is_success() && body.get("status").and_then(|s| s.as_str()) == Some("ok")
                {
                    println!(
                        "Command with PID {} stopped on instance {} (PID {})",
                        pid, info.pid, info.pid
                    );
                    return Ok(true);
                } else {
                    let err_msg = body
                        .get("error")
                        .and_then(|e| e.as_str())
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
