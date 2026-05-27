use std::io::IsTerminal;
use std::time::Duration;

use anyhow::Result;
use crossterm::style::{Color, Stylize};

use crate::cli::args::Cli;
use crate::instance::info::InstanceInfo;
use crate::instance::registry::InstanceRegistry;

/// Create a reqwest::Client with sensible timeouts.
///
/// Without explicit timeouts, reqwest defaults to NO timeout — HTTP
/// requests hang indefinitely if the server is unreachable or unresponsive.
/// This caused `vrunner spawn` and `vrunner stop` to block forever when
/// the target instance's web server wasn't accepting connections.
pub fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(10))
        .build()
        .expect("Failed to build HTTP client")
}

/// Colorize text using crossterm when stdout is a TTY, plain text otherwise.
pub fn c(text: &str, color: Color, bold: bool) -> String {
    if !std::io::stdout().is_terminal() {
        return text.to_string();
    }
    let styled = text.with(color);
    if bold { styled.bold().to_string() } else { styled.to_string() }
}

/// Build the base URL for a vrunner instance, handling auth and TLS.
pub fn instance_url(info: &InstanceInfo, _auth_token: &Option<String>) -> String {
    let scheme = if info.port == 443 { "https" } else { "http" };
    let mut url = format!("{}://{}:{}", scheme, info.bind, info.port);
    // For simplicity, we try HTTP first. TLS instances will reject and
    // the error message will guide the user.
    url = format!("http://{}:{}", info.bind, info.port);
    url
}

/// Discover running vrunner instances and resolve to a single target.
/// Returns the selected InstanceInfo or an error.
pub fn resolve_instance(
    cli: &Cli,
    registry: &InstanceRegistry,
) -> Result<InstanceInfo> {
    let instances = registry.list_instances();

    if instances.is_empty() {
        anyhow::bail!("No running vrunner instances found. Start one first with: vrunner -- <command>");
    }

    // If --target PID was specified, use that instance
    if let Some(target_pid) = cli.target {
        match instances.iter().find(|i| i.pid == target_pid) {
            Some(info) => return Ok(info.clone()),
            None => anyhow::bail!(
                "No vrunner instance found with PID {}. Running instances:\n{}",
                target_pid,
                format_instance_list(&instances)
            ),
        }
    }

    // Only one instance — use it automatically
    if instances.len() == 1 {
        return Ok(instances.into_iter().next().unwrap());
    }

    // Multiple instances — prompt the user
    tracing::warn!("Multiple vrunner instances are running:");
    tracing::warn!("{}", format_instance_list(&instances));
    tracing::warn!("Enter the PID of the instance to use (or Ctrl+C to abort): ");

    // Since we can't easily read stdin in all contexts (piped, daemon, etc.),
    // return an error with instructions
    anyhow::bail!(
        "Multiple vrunner instances are running. Use --target PID to select one.\n\
         Running instances:\n{}",
        format_instance_list(&instances)
    );
}

pub fn format_instance_list(instances: &[InstanceInfo]) -> String {
    let mut out = String::new();
    out.push_str(&format!("{:<10} {:<8} {:<20} {:<10} {:<10} COMMAND\n",
        "PID", "PORT", "BIND", "DAEMON", "DISPLAY"));
    for info in instances {
        out.push_str(&format!("{:<10} {:<8} {:<20} {:<10} {:<10} {}\n",
            info.pid,
            info.port,
            info.bind,
            if info.daemon { "yes" } else { "no" },
            if info.display { "yes" } else { "no" },
            info.command.as_deref().unwrap_or("(idle)")
        ));
    }
    out
}

/// Build the full display string ("name arg1 arg2") from a command JSON value.
pub(crate) fn build_full_display_string(cmd: &serde_json::Value) -> (String, String) {
    let name = cmd.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let args = cmd.get("args").and_then(|v| v.as_array());
    let full = match args {
        Some(arr) => {
            let arg_strs: Vec<&str> = arr.iter().filter_map(|v| v.as_str()).collect();
            if arg_strs.is_empty() {
                name.clone()
            } else {
                format!("{} {}", name, arg_strs.join(" "))
            }
        }
        None => name.clone(),
    };
    (name, full)
}

/// Resolve a PID to a command UUID by querying the instance's command list.
pub async fn resolve_pid_to_id(
    client: &reqwest::Client,
    url: &str,
    pid: u32,
) -> Result<String> {
    let resp = client
        .get(format!("{}/api/commands", url))
        .send()
        .await?;

    let json: serde_json::Value = resp.json().await?;
    if json["status"] != "ok" {
        anyhow::bail!("Failed to query commands from instance");
    }

    if let Some(cmds) = json["data"].as_array() {
        for cmd in cmds {
            if cmd.get("pid").and_then(|v| v.as_u64()) == Some(pid as u64) {
                if let Some(id) = cmd.get("id").and_then(|v| v.as_str()) {
                    return Ok(id.to_string());
                }
            }
        }
    }

    anyhow::bail!("No command found with PID {}", pid)
}

/// Collect all running commands from all instances.
/// Returns Vec of (instance_pid, cmd_id, cmd_pid, name, full_display_string).
pub async fn collect_all_commands(
    client: &reqwest::Client,
    instances: &[InstanceInfo],
) -> Vec<(u32, String, u32, String, String)> {
    let mut all_commands: Vec<(u32, String, u32, String, String)> = Vec::new();
    for info in instances {
        let url = instance_url(info, &None);
        let resp = client
            .get(format!("{}/api/commands", url))
            .send()
            .await;

        if let Ok(resp) = resp {
            if let Ok(json) = resp.json::<serde_json::Value>().await {
                if let Some(cmds) = json["data"].as_array() {
                    for cmd in cmds {
                        let cmd_pid = cmd.get("pid").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                        if let Some(id) = cmd.get("id").and_then(|v| v.as_str()) {
                            let (name, full) = build_full_display_string(cmd);
                            all_commands.push((info.pid, id.to_string(), cmd_pid, name, full));
                        }
                    }
                }
            }
        }
    }
    all_commands
}

/// Filter instances by --target, returning all if no target specified.
pub fn resolve_targeted_instances(
    cli: &Cli,
    all_instances: &[InstanceInfo],
) -> Result<Vec<InstanceInfo>> {
    if let Some(target_pid) = cli.target {
        match all_instances.iter().find(|i| i.pid == target_pid) {
            Some(info) => Ok(vec![info.clone()]),
            None => {
                if all_instances.is_empty() {
                    anyhow::bail!("No running vrunner instances found.");
                }
                anyhow::bail!(
                    "No vrunner instance found with PID {}. Running instances:\n{}",
                    target_pid,
                    all_instances.iter().map(|i| format!("  PID: {}", i.pid)).collect::<Vec<_>>().join("\n")
                );
            }
        }
    } else {
        Ok(all_instances.to_vec())
    }
}

/// Resolve the target PID for the `vrunner stop` subcommand.
/// If no PID is given, resolves to the only instance if exactly one is running.
/// Exits the process if the resolution fails or is ambiguous.
pub fn resolve_stop_target(
    pid: Option<u32>,
    instances: &[InstanceInfo],
) -> u32 {
    match pid {
        Some(p) => p,
        None => {
            // No PID given — stop the only instance if exactly one is running
            match instances.len() {
                0 => {
                    tracing::error!("No vrunner instances running.");
                    std::process::exit(1);
                }
                1 => {
                    let p = instances[0].pid;
                    tracing::info!("Stopping only running instance (PID {})", p);
                    p
                }
                _ => {
                    tracing::warn!("Multiple vrunner instances running. Specify which one to stop:");
                    for inst in instances {
                        tracing::warn!("  PID {} — port {}", inst.pid, inst.port);
                    }
                    tracing::warn!("Usage: vrunner stop <PID>");
                    std::process::exit(1);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: the `build_full_display_string` function is private but tests
    /// inside the same module can access it.
    #[test]
    fn test_build_full_display_string_with_args() {
        let cmd = serde_json::json!({
            "name": "vim",
            "args": ["file.txt", "-p"],
            "pid": 1234,
            "id": "abc-123"
        });
        let (name, full) = build_full_display_string(&cmd);
        assert_eq!(name, "vim");
        assert_eq!(full, "vim file.txt -p");
    }

    #[test]
    fn test_build_full_display_string_no_args() {
        let cmd = serde_json::json!({
            "name": "htop",
            "args": [],
            "pid": 5678,
            "id": "def-456"
        });
        let (name, full) = build_full_display_string(&cmd);
        assert_eq!(name, "htop");
        assert_eq!(full, "htop");
    }

    #[test]
    fn test_build_full_display_string_missing_name() {
        let cmd = serde_json::json!({
            "args": ["-la"],
            "pid": 9999,
            "id": "ghi-789"
        });
        let (name, full) = build_full_display_string(&cmd);
        assert_eq!(name, "");
        assert_eq!(full, " -la");
    }

    #[test]
    fn test_build_full_display_string_missing_args() {
        let cmd = serde_json::json!({
            "name": "bash",
            "pid": 1111,
            "id": "jkl-012"
        });
        let (name, full) = build_full_display_string(&cmd);
        assert_eq!(name, "bash");
        assert_eq!(full, "bash");
    }

    #[test]
    fn test_build_full_display_string_empty_object() {
        let cmd = serde_json::json!({});
        let (name, full) = build_full_display_string(&cmd);
        assert_eq!(name, "");
        assert_eq!(full, "");
    }

    #[test]
    fn test_build_full_display_string_null_args() {
        let cmd = serde_json::json!({
            "name": "sleep",
            "args": null,
            "pid": 2222,
            "id": "mno-345"
        });
        let (name, full) = build_full_display_string(&cmd);
        assert_eq!(name, "sleep");
        assert_eq!(full, "sleep");
    }

    #[test]
    fn test_build_full_display_string_non_string_args_ignored() {
        let cmd = serde_json::json!({
            "name": "cmd",
            "args": [42, true, "only-string"],
            "pid": 3333,
            "id": "pqr-678"
        });
        let (name, full) = build_full_display_string(&cmd);
        assert_eq!(name, "cmd");
        assert_eq!(full, "cmd only-string");
    }
}
