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
pub async fn resolve_pid_to_id(vrw: &VrwClient, pid: u32) -> Result<String> {
    let data = vrw.get_data("/api/commands").await?;

    if let Some(cmds) = data.as_array() {
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
        let vrw = VrwClient::new(client.clone(), info);
        if let Some(data) = vrw.try_get_data("/api/commands").await {
            if let Some(cmds) = data.as_array() {
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
#[cfg(feature = "vrw")]
mod vrw_tests {
    use clap::Parser;
    use super::*;
    use crate::cli::args::BINARY_NAME;

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
    fn test_resolve_target_command_no_match() {
        let cmds: Vec<CommandTarget> = vec![
            (100, "id-aaa".into(), 1234, "vim".into(), "vim file.txt".into()),
        ];
        assert!(resolve_target_command(Some("nonexistent"), &cmds, "No command").is_err());
    }

    #[test]
    fn test_instance_url_standard_port() {
        let info = InstanceInfo {
            pid: 1234,
            port: 9090,
            bind: "127.0.0.1".to_string(),
            name: Some("test".to_string()),
            start_time: chrono::Utc::now(),
            daemon: false,
            display: false,
            command: None,
        };
        let url = instance_url(&info, &None);
        assert_eq!(url, "http://127.0.0.1:9090");
    }

    #[test]
    fn test_format_instance_list_empty() {
        let instances: Vec<InstanceInfo> = vec![];
        let output = format_instance_list(&instances);
        // Header should be present
        assert!(output.contains("PID"));
        assert!(output.contains("PORT"));
        assert!(output.contains("COMMAND"));
    }

    #[test]
    fn test_format_instance_list_single() {
        let info = InstanceInfo {
            pid: 1234,
            port: 9090,
            bind: "127.0.0.1".to_string(),
            name: Some("main".to_string()),
            start_time: chrono::Utc::now(),
            daemon: false,
            display: true,
            command: Some("htop".to_string()),
        };
        let output = format_instance_list(&[info]);
        assert!(output.contains("1234"));
        assert!(output.contains("9090"));
        assert!(output.contains("127.0.0.1"));
        assert!(output.contains("main"));
        assert!(output.contains("htop"));
    }

    #[test]
    fn test_format_instance_list_multiple() {
        let a = InstanceInfo {
            pid: 100,
            port: 9090,
            bind: "127.0.0.1".to_string(),
            name: None,
            start_time: chrono::Utc::now(),
            daemon: true,
            display: false,
            command: None,
        };
        let b = InstanceInfo {
            pid: 200,
            port: 9091,
            bind: "0.0.0.0".to_string(),
            name: Some("web".to_string()),
            start_time: chrono::Utc::now(),
            daemon: false,
            display: true,
            command: Some("vim".to_string()),
        };
        let output = format_instance_list(&[a, b]);
        assert!(output.contains("100"));
        assert!(output.contains("200"));
        assert!(output.contains("web"));
    }

    #[test]
    fn test_format_instance_list_defaults() {
        let info = InstanceInfo {
            pid: 1,
            port: 8080,
            bind: "localhost".to_string(),
            name: None,
            start_time: chrono::Utc::now(),
            daemon: false,
            display: false,
            command: None,
        };
        let output = format_instance_list(&[info]);
        // Name defaults to "-", command defaults to "(idle)"
        assert!(output.contains("-") && output.contains("(idle)"));
    }

    #[test]
    fn test_resolve_targeted_instances_no_pid_returns_all() {
        let instances = vec![make_test_instance(100, 9090), make_test_instance(200, 9091)];
        let cli = crate::cli::args::Cli::try_parse_from([BINARY_NAME]).unwrap();
        let result = resolve_targeted_instances(&cli, &instances).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_resolve_targeted_instances_matching_pid() {
        let instances = vec![make_test_instance(100, 9090), make_test_instance(200, 9091)];
        let cli = crate::cli::args::Cli::try_parse_from([BINARY_NAME, "--pid", "200"]).unwrap();
        let result = resolve_targeted_instances(&cli, &instances).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].pid, 200);
    }

    #[test]
    fn test_resolve_targeted_instances_nonexistent_pid_errors() {
        let instances = vec![make_test_instance(100, 9090)];
        let cli = crate::cli::args::Cli::try_parse_from([BINARY_NAME, "--pid", "999"]).unwrap();
        let result = resolve_targeted_instances(&cli, &instances);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("999"));
    }

    #[test]
    fn test_resolve_targeted_instances_no_instances_errors() {
        let instances: Vec<InstanceInfo> = vec![];
        let cli = crate::cli::args::Cli::try_parse_from([BINARY_NAME, "--pid", "1"]).unwrap();
        let result = resolve_targeted_instances(&cli, &instances);
        assert!(result.is_err());
    }

    fn make_test_instance(pid: u32, port: u16) -> InstanceInfo {
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

/// Typed HTTP client for vrw CLI commands.
///
/// Encapsulates the reqwest client and base URL, providing typed methods
/// that handle response parsing centrally.  Eliminates the inline
/// HTTP boilerplate that was previously copy-pasted across every command.
#[cfg(feature = "vrw")]
pub struct VrwClient {
    client: reqwest::Client,
    base_url: String,
    instance_pid: u32,
}

#[cfg(feature = "vrw")]
impl VrwClient {
    pub fn new(client: reqwest::Client, info: &InstanceInfo) -> Self {
        Self {
            base_url: instance_url(info, &None),
            instance_pid: info.pid,
            client,
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn instance_pid(&self) -> u32 {
        self.instance_pid
    }

    /// Return a reference to the underlying reqwest client.
    pub fn client(&self) -> &reqwest::Client {
        &self.client
    }

    /// GET a JSON endpoint, returning None on any error (connection refused, etc.).
    /// Used for collection/discovery where individual failures should be silently skipped.
    pub async fn try_get_data(&self, path: &str) -> Option<serde_json::Value> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self.client.get(&url).send().await.ok()?;
        if !resp.status().is_success() {
            return None;
        }
        let json: serde_json::Value = resp.json().await.ok()?;
        if json.get("status").and_then(|s| s.as_str()) == Some("ok") {
            json.get("data").cloned()
        } else {
            None
        }
    }

    /// GET a JSON endpoint, returning the `data` field on success.
    pub async fn get_data(&self, path: &str) -> Result<serde_json::Value> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self.client.get(&url).send().await?;
        let status = resp.status();
        let json: serde_json::Value = resp.json().await
            .unwrap_or(serde_json::json!({"status": "unknown"}));
        if status.is_success() && json.get("status").and_then(|s| s.as_str()) == Some("ok") {
            Ok(json.get("data").cloned().unwrap_or(serde_json::Value::Null))
        } else {
            let err = json.get("error").and_then(|e| e.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("HTTP {}", status));
            anyhow::bail!("{}", err)
        }
    }

    /// GET raw bytes from an endpoint (for screenshots etc).
    pub async fn get_bytes(&self, path: &str) -> Result<bytes::Bytes> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self.client.get(&url).send().await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("HTTP {}: {}", status, body);
        }
        Ok(resp.bytes().await?)
    }

    /// POST to a command action endpoint, check for ok status.
    pub async fn post_action(&self, path: &str, body: Option<&serde_json::Value>) -> Result<()> {
        let url = format!("{}{}", self.base_url, path);
        let req = match body {
            Some(b) => self.client.post(&url).json(b),
            None => self.client.post(&url),
        };
        let resp = req.send().await?;
        let status = resp.status();
        let json: serde_json::Value = resp.json().await
            .unwrap_or(serde_json::json!({"status": "unknown"}));
        if status.is_success() && json.get("status").and_then(|s| s.as_str()) == Some("ok") {
            Ok(())
        } else {
            let err = json.get("error").and_then(|e| e.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("HTTP {}", status));
            anyhow::bail!("{}", err)
        }
    }

    /// POST and return the `data` field from the response.
    pub async fn post_data(&self, path: &str, body: &serde_json::Value) -> Result<serde_json::Value> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self.client.post(&url).json(body).send().await?;
        let status = resp.status();
        let json: serde_json::Value = resp.json().await
            .unwrap_or(serde_json::json!({"status": "unknown"}));
        if status.is_success() && json.get("status").and_then(|s| s.as_str()) == Some("ok") {
            Ok(json.get("data").cloned().unwrap_or(serde_json::Value::Null))
        } else {
            let err = json.get("error").and_then(|e| e.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("HTTP {}", status));
            anyhow::bail!("{}", err)
        }
    }

    /// POST to a command action endpoint, printing success_msg on ok,
    /// logging error on failure. Returns Ok(false) on failure.
    pub async fn post_action_quiet(
        &self,
        path: &str,
        body: Option<&serde_json::Value>,
        method: reqwest::Method,
        label: &str,
        verb: &str,
        success_msg: &str,
    ) -> Result<bool> {
        let url = format!("{}{}", self.base_url, path);
        let req = match body {
            Some(b) => self.client.request(method.clone(), &url).json(b),
            None => self.client.request(method, &url),
        };
        match req.send().await {
            Ok(resp) => {
                let status = resp.status();
                let json: serde_json::Value = resp.json().await
                    .unwrap_or(serde_json::json!({"status": "unknown"}));
                if status.is_success() && json.get("status").and_then(|s| s.as_str()) == Some("ok") {
                    println!("{}", success_msg);
                    Ok(true)
                } else {
                    let err = json.get("error").and_then(|e| e.as_str())
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| format!("HTTP {}", status));
                    tracing::error!("Failed to {} '{}': {}", verb, label, err);
                    Ok(false)
                }
            }
            Err(e) => {
                tracing::error!("Failed to {} '{}': {}", verb, label, e);
                Ok(false)
            }
        }
    }
}

/// Label style for [`build_command_select_items`].
#[cfg(feature = "vrw")]
#[derive(Clone, Copy)]
pub enum SelectLabelStyle {
    /// `"full_display (PID n)"`
    FullWithPid,
    /// `"id_prefix — full_display"`
    IdPrefixWithFull,
}

/// Build a list of [`crate::cli::interactive_select::SelectItem`] from a
/// command list, using the given label style.
#[cfg(feature = "vrw")]
pub fn build_command_select_items(
    all_commands: &[CommandTarget],
    style: SelectLabelStyle,
) -> Vec<crate::cli::interactive_select::SelectItem> {
    all_commands
        .iter()
        .map(|(_, id, pid, _, full)| {
            let label = match style {
                SelectLabelStyle::FullWithPid => format!("{} (PID {})", full, pid),
                SelectLabelStyle::IdPrefixWithFull => {
                    format!("{} — {}", &id[..8.min(id.len())], full)
                }
            };
            crate::cli::interactive_select::SelectItem {
                label,
                id: id.clone(),
            }
        })
        .collect()
}

/// Collect commands from all instances, keeping only those for which `filter`
/// returns `true`.  This replaces the per-file `collect_running_commands`,
/// `collect_kept_commands`, and the inline collection in purge.rs.
#[cfg(feature = "vrw")]
pub async fn collect_filtered_commands<F>(
    client: &reqwest::Client,
    instances: &[InstanceInfo],
    filter: F,
) -> Vec<CommandTarget>
where
    F: Fn(&serde_json::Value) -> bool,
{
    let mut result = Vec::new();
    for info in instances {
        let vrw = VrwClient::new(client.clone(), info);
        if let Some(data) = vrw.try_get_data("/api/commands").await {
            if let Some(cmds) = data.as_array() {
                for cmd in cmds {
                    if !filter(cmd) {
                        continue;
                    }
                    let cmd_pid = cmd.get("pid").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                    if let Some(id) = cmd.get("id").and_then(|v| v.as_str()) {
                        let (name, full) = build_full_display_string(cmd);
                        result.push((info.pid, id.to_string(), cmd_pid, name, full));
                    }
                }
            }
        }
    }
    result
}
