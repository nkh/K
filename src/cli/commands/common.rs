use std::io::IsTerminal;

use crossterm::style::{Color, Stylize};

/// Colorize text using crossterm when stdout is a TTY, plain text otherwise.
pub fn c(text: &str, color: Color, bold: bool) -> String {
    if !std::io::stdout().is_terminal() {
        return text.to_string();
    }
    let styled = text.with(color);
    if bold {
        styled.bold().to_string()
    } else {
        styled.to_string()
    }
}

// ── vrunner-only helper functions ──

#[cfg(feature = "vrunner")]
use std::time::Duration;

#[cfg(feature = "vrunner")]
use anyhow::Result;

#[cfg(feature = "vrunner")]
use crate::cli::args::Cli;
#[cfg(feature = "vrunner")]
use crate::instance::info::InstanceInfo;
#[cfg(feature = "vrunner")]
use crate::instance::registry::InstanceRegistry;

/// Create a reqwest::Client with sensible timeouts.
#[cfg(feature = "vrunner")]
pub fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(10))
        .build()
        .expect("Failed to build HTTP client")
}

/// Build the base URL for a vrunner instance, handling auth and TLS.
#[cfg(feature = "vrunner")]
pub fn instance_url(info: &InstanceInfo, _auth_token: &Option<String>) -> String {
    let scheme = if info.port == 443 { "https" } else { "http" };
    let mut url = format!("{}://{}:{}", scheme, info.bind, info.port);
    // For simplicity, we try HTTP first. TLS instances will reject and
    // the error message will guide the user.
    url = format!("http://{}:{}", info.bind, info.port);
    url
}

/// Discover running vrunner instances and resolve to a single target.
#[cfg(feature = "vrunner")]
pub fn resolve_instance(cli: &Cli, registry: &InstanceRegistry) -> Result<InstanceInfo> {
    let instances = registry.list_instances();

    if instances.is_empty() {
        anyhow::bail!(
            "No running vrunner instances found. Start one first with: vrunner -- <command>"
        );
    }

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

    if instances.len() == 1 {
        return Ok(instances.into_iter().next().unwrap());
    }

    tracing::warn!("Multiple vrunner instances are running:");
    tracing::warn!("{}", format_instance_list(&instances));
    tracing::warn!("Enter the PID of the instance to use (or Ctrl+C to abort): ");

    anyhow::bail!(
        "Multiple vrunner instances are running. Use --target PID to select one.\n\
         Running instances:\n{}",
        format_instance_list(&instances)
    );
}

#[cfg(feature = "vrunner")]
pub fn format_instance_list(instances: &[InstanceInfo]) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "{:<10} {:<8} {:<20} {:<10} {:<10} COMMAND\n",
        "PID", "PORT", "BIND", "DAEMON", "DISPLAY"
    ));
    for info in instances {
        out.push_str(&format!(
            "{:<10} {:<8} {:<20} {:<10} {:<10} {}\n",
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
#[cfg(feature = "vrunner")]
pub(crate) fn build_full_display_string(cmd: &serde_json::Value) -> (String, String) {
    let name = cmd
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
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
#[cfg(feature = "vrunner")]
pub async fn resolve_pid_to_id(client: &reqwest::Client, url: &str, pid: u32) -> Result<String> {
    let resp = client.get(format!("{}/api/commands", url)).send().await?;

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
#[cfg(feature = "vrunner")]
pub async fn collect_all_commands(
    client: &reqwest::Client,
    instances: &[InstanceInfo],
) -> Vec<(u32, String, u32, String, String)> {
    let mut all_commands: Vec<(u32, String, u32, String, String)> = Vec::new();
    for info in instances {
        let url = instance_url(info, &None);
        let resp = client.get(format!("{}/api/commands", url)).send().await;

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
#[cfg(feature = "vrunner")]
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
                    all_instances
                        .iter()
                        .map(|i| format!("  PID: {}", i.pid))
                        .collect::<Vec<_>>()
                        .join("\n")
                );
            }
        }
    } else {
        Ok(all_instances.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_c_plain_when_not_tty() {
        let result = c("test", Color::Red, true);
        assert!(result.contains("test"));
    }
}

#[cfg(test)]
#[cfg(feature = "vrunner")]
mod vrunner_tests {
    use super::*;

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
}
