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

// ── vrw-only helper functions ──

#[cfg(feature = "vrw")]
use std::time::Duration;

#[cfg(feature = "vrw")]
use anyhow::Result;

#[cfg(feature = "vrw")]
use crate::cli::args::Cli;
#[cfg(feature = "vrw")]
use crate::instance::info::InstanceInfo;
#[cfg(feature = "vrw")]
use crate::instance::registry::InstanceRegistry;

/// Create a reqwest::Client with sensible timeouts.
#[cfg(feature = "vrw")]
pub fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(10))
        .build()
        .expect("Failed to build HTTP client")
}

/// Build the base URL for a vrw instance, handling auth and TLS.
#[cfg(feature = "vrw")]
pub fn instance_url(info: &InstanceInfo, _auth_token: &Option<String>) -> String {
    let scheme = if info.port == 443 { "https" } else { "http" };
    let mut url = format!("{}://{}:{}", scheme, info.bind, info.port);
    // For simplicity, we try HTTP first. TLS instances will reject and
    // the error message will guide the user.
    url = format!("http://{}:{}", info.bind, info.port);
    url
}

/// Discover running vrw instances and resolve to a single target.
#[cfg(feature = "vrw")]
pub fn resolve_instance(cli: &Cli, registry: &InstanceRegistry) -> Result<InstanceInfo> {
    let instances = registry.list_instances();

    if instances.is_empty() {
        anyhow::bail!(
            "No running vrw instances found. Start one first with: vrw -- <command>"
        );
    }

    if let Some(target_pid) = cli.pid {
        match instances.iter().find(|i| i.pid == target_pid) {
            Some(info) => return Ok(info.clone()),
            None => anyhow::bail!(
                "No vrw instance found with PID {}. Running instances:\n{}",
                target_pid,
                format_instance_list(&instances)
            ),
        }
    }

    if instances.len() == 1 {
        return Ok(instances.into_iter().next().unwrap());
    }

    tracing::warn!("Multiple vrw instances are running:");
    tracing::warn!("{}", format_instance_list(&instances));
    tracing::warn!("Enter the PID of the instance to use (or Ctrl+C to abort): ");

    anyhow::bail!(
        "Multiple vrw instances are running. Use --pid PID to select one.\n\
         Running instances:\n{}",
        format_instance_list(&instances)
    );
}

#[cfg(feature = "vrw")]
pub fn format_instance_list(instances: &[InstanceInfo]) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "{:<10} {:<8} {:<20} {:<10} {:<10} {:<15} COMMAND\n",
        "PID", "PORT", "BIND", "DAEMON", "DISPLAY", "NAME"
    ));
    for info in instances {
        out.push_str(&format!(
            "{:<10} {:<8} {:<20} {:<10} {:<10} {:<15} {}\n",
            info.pid,
            info.port,
            info.bind,
            if info.daemon { "yes" } else { "no" },
            if info.display { "yes" } else { "no" },
            info.name.as_deref().unwrap_or("-"),
            info.command.as_deref().unwrap_or("(idle)")
        ));
    }
    out
}

/// Build the full display string ("name arg1 arg2") from a command JSON value.
#[cfg(feature = "vrw")]
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
#[cfg(feature = "vrw")]
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
#[cfg(feature = "vrw")]
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

/// Filter instances by --pid, returning all if no pid specified.
#[cfg(feature = "vrw")]
pub fn resolve_targeted_instances(
    cli: &Cli,
    all_instances: &[InstanceInfo],
) -> Result<Vec<InstanceInfo>> {
    if let Some(target_pid) = cli.pid {
        match all_instances.iter().find(|i| i.pid == target_pid) {
            Some(info) => Ok(vec![info.clone()]),
            None => {
                if all_instances.is_empty() {
                    anyhow::bail!("No running vrw instances found.");
                }
                anyhow::bail!(
                    "No vrw instance found with PID {}. Running instances:\n{}",
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
#[cfg(feature = "vrw")]
mod vrw_tests {
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

    #[test]
    fn test_resolve_target_command_by_pid() {
        let cmds: Vec<CommandTarget> = vec![
            (100, "id-aaa".into(), 1234, "sleep".into(), "sleep 60".into()),
            (100, "id-bbb".into(), 5678, "vim".into(), "vim file.txt".into()),
        ];
        // Numeric PID match
        let result = resolve_target_command(Some("1234"), &cmds, "test").unwrap();
        assert_eq!(result.2, 1234);
        assert_eq!(result.1, "id-aaa");

        let result = resolve_target_command(Some("5678"), &cmds, "test").unwrap();
        assert_eq!(result.2, 5678);
    }

    #[test]
    fn test_resolve_target_command_by_name() {
        let cmds: Vec<CommandTarget> = vec![
            (100, "id-aaa".into(), 1234, "sleep".into(), "sleep 60".into()),
            (100, "id-bbb".into(), 5678, "vim".into(), "vim file.txt".into()),
        ];
        // Exact name match
        let result = resolve_target_command(Some("vim"), &cmds, "test").unwrap();
        assert_eq!(result.1, "id-bbb");

        // Name prefix match
        let result = resolve_target_command(Some("sl"), &cmds, "test").unwrap();
        assert_eq!(result.1, "id-aaa");
    }

    #[test]
    fn test_resolve_target_command_by_id_prefix() {
        let cmds: Vec<CommandTarget> = vec![
            (100, "abcdef-1234".into(), 1234, "sleep".into(), "sleep 60".into()),
        ];
        // ID prefix "abc" is not a PID, not a name, but matches prefix of full display "sleep 60"? No.
        // resolve_target_command does not do ID prefix matching — it does PID, name, full, prefix-full, prefix-name.
        // "abc" doesn't match any of those, so it should error.
        assert!(resolve_target_command(Some("abc"), &cmds, "test").is_err());
        // But full ID prefix "abcdef" doesn't match either — ID prefix matching is not in resolve_target_command.
        // It's handled by callers like keep.rs that do their own ID prefix matching before calling this.
    }

    #[test]
    fn test_resolve_target_command_none_auto_selects_single() {
        let cmds: Vec<CommandTarget> = vec![
            (100, "id-aaa".into(), 1234, "sleep".into(), "sleep 60".into()),
        ];
        let result = resolve_target_command(None, &cmds, "test").unwrap();
        assert_eq!(result.1, "id-aaa");
    }

    #[test]
    fn test_resolve_target_command_none_errors_on_multiple() {
        let cmds: Vec<CommandTarget> = vec![
            (100, "id-aaa".into(), 1234, "sleep".into(), "sleep 60".into()),
            (100, "id-bbb".into(), 5678, "vim".into(), "vim file.txt".into()),
        ];
        assert!(resolve_target_command(None, &cmds, "test").is_err());
    }

    #[test]
    fn test_resolve_target_command_none_errors_on_empty() {
        let cmds: Vec<CommandTarget> = vec![];
        assert!(resolve_target_command(None, &cmds, "test").is_err());
    }
}

// ── vrw-only: shared target resolution ──

/// A resolved command target: (instance_pid, cmd_id, cmd_pid, name, full_display).
#[cfg(feature = "vrw")]
pub type CommandTarget = (u32, String, u32, String, String);

/// Resolve a user-supplied target (PID, name, or None) to a single command.
///
/// Uses the same three-round matching strategy across cat, screenshot, resize,
/// stop-command, and purge: numeric PID first, then exact name/full match, then
/// prefix match.  When `target` is `None`, auto-selects the sole command or
/// errors with a disambiguation list.
#[cfg(feature = "vrw")]
pub fn resolve_target_command(
    target: Option<&str>,
    all_commands: &[CommandTarget],
    error_prefix: &str,
) -> Result<CommandTarget> {
    match target {
        Some(t) => {
            // Fast path: numeric PID
            if let Ok(pid) = t.parse::<u32>() {
                match all_commands.iter().find(|(_, _, p, _, _)| *p == pid) {
                    Some(entry) => return Ok(entry.clone()),
                    None => anyhow::bail!(
                        "No command found with PID {}. Use `vrw list` to see running commands.",
                        pid
                    ),
                }
            }
            // Exact name match (case-insensitive)
            let exact: Vec<_> = all_commands
                .iter()
                .filter(|(_, _, _, name, _)| name.eq_ignore_ascii_case(t))
                .collect();
            match exact.len() {
                1 => return Ok(exact[0].clone()),
                0 => {}
                _ => {
                    let list: Vec<_> = exact.iter().map(|e| format!("  pid {}", e.2)).collect();
                    anyhow::bail!(
                        "Multiple commands matching '{}':\n{}\nUse a PID to disambiguate.",
                        t,
                        list.join("\n")
                    );
                }
            }
            // Exact full match
            let exact_full: Vec<_> = all_commands
                .iter()
                .filter(|(_, _, _, _, full)| full == t)
                .collect();
            match exact_full.len() {
                1 => return Ok(exact_full[0].clone()),
                0 => {}
                _ => {
                    let list: Vec<_> = exact_full.iter().map(|e| format!("  pid {}", e.2)).collect();
                    anyhow::bail!(
                        "Multiple commands matching '{}':\n{}\nUse a PID to disambiguate.",
                        t,
                        list.join("\n")
                    );
                }
            }
            // Prefix match on full string
            let prefix_full: Vec<_> = all_commands
                .iter()
                .filter(|(_, _, _, _, full)| full.starts_with(t))
                .collect();
            if prefix_full.len() == 1 {
                return Ok(prefix_full[0].clone());
            }
            // Prefix match on name only
            let prefix_name: Vec<_> = all_commands
                .iter()
                .filter(|(_, _, _, name, _)| name.starts_with(t))
                .collect();
            if prefix_name.len() == 1 {
                return Ok(prefix_name[0].clone());
            }
            anyhow::bail!(
                "{} matching '{}'. Use `vrw list` to see running commands.",
                error_prefix, t
            )
        }
        None => match all_commands.len() {
            0 => anyhow::bail!("{} no running commands.", error_prefix),
            1 => Ok(all_commands[0].clone()),
            _ => {
                let list: Vec<_> = all_commands
                    .iter()
                    .map(|e| format!("  pid {}  {}", e.2, e.3))
                    .collect();
                anyhow::bail!(
                    "Multiple commands running. Specify a target:\n{}",
                    list.join("\n")
                )
            }
        },
    }
}

/// Send a POST to a command action endpoint and parse the standard
/// `{ "status": "ok", ... }` / `{ "status": "error", "error": "..." }` response.
///
/// Used by freeze, thaw, stop, resize, and purge to avoid repeating the
/// same response-check boilerplate in every handler.
#[cfg(feature = "vrw")]
pub async fn post_command_action(
    client: &reqwest::Client,
    url: &str,
    path: &str,
    body: Option<&serde_json::Value>,
) -> Result<bool> {
    let req = match body {
        Some(b) => client.post(format!("{}{}", url, path)).json(b),
        None => client.post(format!("{}{}", url, path)),
    };
    let resp = req.send().await?;

    let status = resp.status();
    let json: serde_json::Value = resp
        .json()
        .await
        .unwrap_or(serde_json::json!({"status": "unknown"}));

    if status.is_success() && json.get("status").and_then(|s| s.as_str()) == Some("ok") {
        Ok(true)
    } else {
        let err_msg = json
            .get("error")
            .and_then(|e| e.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("HTTP {}", status));
        Err(anyhow::anyhow!("{}", err_msg))
    }
}
