use anyhow::Result;

use crate::instance::info::InstanceInfo;

// ── vrc (signal-based) implementation ──

/// Stop a vrc instance by sending SIGTERM to its PID.
#[cfg(not(feature = "vrw"))]
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
        println!("Sent SIGTERM to vrc instance (PID {})", target_pid);
    }
    #[cfg(not(unix))]
    {
        std::process::Command::new("kill")
            .arg(target_pid.to_string())
            .spawn()?
            .wait()?;
        println!("Sent kill signal to vrc instance (PID {})", target_pid);
    }

    Ok(())
}

/// Resolve the target PID for the `stop` subcommand.
///
/// Shared core logic: when `pid` is `None`, auto-selects the sole instance or
/// errors with a disambiguation list.  The `binary_name` and `extra_info`
/// closure allow each binary to customise its messages.
#[cfg(not(feature = "vrw"))]
pub fn resolve_stop_target(pid: Option<u32>, instances: &[InstanceInfo]) -> u32 {
    resolve_stop_target_impl(pid, instances, "vrc", |_| String::new())
}

#[cfg(feature = "vrw")]
pub fn resolve_stop_target(pid: Option<u32>, instances: &[InstanceInfo]) -> u32 {
    resolve_stop_target_impl(pid, instances, "vrw", |inst| {
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

// ── vrw (HTTP-based) implementation ──

#[cfg(feature = "vrw")]
use super::common::{
    build_command_select_items, collect_all_commands, http_client, instance_url,
    post_command_action_bool, resolve_target_command, SelectLabelStyle,
};
#[cfg(feature = "vrw")]
use crate::cli::interactive_select::select_items;

/// Stop a specific command by PID or name on any running instance (vrw).
#[cfg(feature = "vrw")]
pub async fn handle_stop_command(_cli: &crate::cli::args::Cli, target: Option<&str>, interactive: bool) -> Result<bool> {
    let registry = crate::instance::registry::InstanceRegistry::new()?;
    let instances = registry.list_instances();

    if instances.is_empty() {
        return Ok(false);
    }

    let client = http_client();
    let all_commands = collect_all_commands(&client, &instances).await;

    if all_commands.is_empty() {
        tracing::warn!("No commands running.");
        return Ok(false);
    }

    // Interactive mode: list all commands and let user select
    if interactive && target.is_none() {
        let items = build_command_select_items(&all_commands, SelectLabelStyle::FullWithPid);
        let selected = select_items(&items, "Select commands to stop [space-separated numbers]")?;
        let mut any_stopped = false;
        for item in &selected {
            if let Some((inst_pid, cmd_id, cmd_pid, _, _)) = all_commands.iter().find(|(_, id, _, _, _)| id == &item.id) {
                let info = instances.iter().find(|i| i.pid == *inst_pid).unwrap();
                let url = instance_url(info, &None);
                if stop_command_by_id(&client, &url, cmd_id, *cmd_pid, *inst_pid).await? {
                    any_stopped = true;
                }
            }
        }
        return Ok(any_stopped);
    }

    // Use standard target resolution (PID, name, prefix)
    match resolve_target_command(target, &all_commands, "No running command") {
        Ok((inst_pid, cmd_id, cmd_pid, _, full)) => {
            let info = instances.iter().find(|i| i.pid == inst_pid).unwrap();
            let url = instance_url(info, &None);
            tracing::info!("Stopping: {} (PID {})", full, cmd_pid);
            stop_command_by_id(&client, &url, &cmd_id, cmd_pid, inst_pid).await
        }
        Err(_) => Ok(false),
    }
}

/// Stop all commands across all running instances (vrw).
#[cfg(feature = "vrw")]
pub async fn handle_stop_all_commands(_cli: &crate::cli::args::Cli) -> Result<bool> {
    let registry = crate::instance::registry::InstanceRegistry::new()?;
    let instances = registry.list_instances();

    if instances.is_empty() {
        return Ok(false);
    }

    let client = http_client();
    let mut any_stopped = false;

    for info in &instances {
        let url = instance_url(info, &None);
        let all_commands = collect_all_commands(&client, std::slice::from_ref(info)).await;
        for (_, cmd_id, cmd_pid, _, _) in &all_commands {
            if stop_command_by_id(&client, &url, cmd_id, *cmd_pid, info.pid).await? {
                any_stopped = true;
            }
        }
    }

    Ok(any_stopped)
}

#[cfg(feature = "vrw")]
pub async fn stop_command_by_id(
    client: &reqwest::Client,
    url: &str,
    cmd_id: &str,
    cmd_pid: u32,
    inst_pid: u32,
) -> Result<bool> {
    post_command_action_bool(
        client,
        url,
        &format!("/api/commands/{}/kill", cmd_id),
        Some(&serde_json::json!({})),
        reqwest::Method::POST,
        &format!("command with PID {}", cmd_pid),
        "stop",
        &format!(
            "Command with PID {} stopped on instance {} (PID {})",
            cmd_pid, inst_pid, inst_pid
        ),
    )
    .await
}

#[cfg(test)]
mod tests {
    use clap::Parser;
    use super::*;

    /// Helper to create an InstanceInfo for testing.
    #[cfg(feature = "vrw")]
    fn make_instance(pid: u32, port: u16) -> InstanceInfo {
        InstanceInfo {
            pid,
            port,
            bind: "127.0.0.1".to_string(),
            name: None,
            start_time: chrono::Utc::now(),
            daemon: false,
            display: false,
            command: None,
        }
    }

    #[cfg(not(feature = "vrw"))]
    fn make_instance(pid: u32) -> InstanceInfo {
        InstanceInfo {
            pid,
            start_time: chrono::Utc::now(),
            daemon: false,
            display: false,
        }
    }

    #[test]
    fn resolve_explicit_pid_returned() {
        // When an explicit PID is given, it should be returned as-is
        #[cfg(feature = "vrw")]
        let instances = vec![make_instance(9999, 9090)];
        #[cfg(not(feature = "vrw"))]
        let instances = vec![make_instance(9999)];
        let result = resolve_stop_target(Some(42), &instances);
        assert_eq!(result, 42);
    }

    #[test]
    fn resolve_single_instance_auto_selected() {
        // When no PID is given and only one instance exists, return its PID
        #[cfg(feature = "vrw")]
        let instances = vec![make_instance(1234, 9090)];
        #[cfg(not(feature = "vrw"))]
        let instances = vec![make_instance(1234)];
        let result = resolve_stop_target(None, &instances);
        assert_eq!(result, 1234);
    }

    #[test]
    fn resolve_explicit_pid_ignored_instances() {
        // Explicit PID should be returned regardless of what instances exist
        #[cfg(feature = "vrw")]
        let instances = vec![make_instance(1111, 9090), make_instance(2222, 9091)];
        #[cfg(not(feature = "vrw"))]
        let instances = vec![make_instance(1111), make_instance(2222)];
        assert_eq!(resolve_stop_target(Some(5000), &instances), 5000);
    }

    #[cfg(feature = "vrw")]
    #[tokio::test]
    async fn test_stop_command_by_id_connection_refused() {
        use super::super::common::http_client;
        let client = http_client();
        let result = stop_command_by_id(&client, "http://127.0.0.1:1", "fake-id", 100, 200).await;
        assert!(result.is_ok());
        assert!(!result.unwrap(), "connection refused should return false");
    }

    #[cfg(feature = "vrw")]
    #[tokio::test]
    async fn test_handle_stop_all_commands_no_instances() {
        let cli = crate::cli::args::Cli::try_parse_from(["vrw", "stop-command", "--all"]).unwrap();
        let result = handle_stop_all_commands(&cli).await;
        assert!(result.is_ok());
        assert!(!result.unwrap(), "no instances should return false");
    }

    #[cfg(feature = "vrw")]
    #[tokio::test]
    async fn test_handle_stop_command_no_instances() {
        let cli = crate::cli::args::Cli::try_parse_from(["vrw", "stop-command"]).unwrap();
        let result = handle_stop_command(&cli, None, false).await;
        assert!(result.is_ok());
        assert!(!result.unwrap(), "no instances should return false");
    }
}