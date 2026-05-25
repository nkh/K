use anyhow::Result;
use crossterm::style::{Color, Stylize};
use std::io::IsTerminal;

use crate::cli::args::{CertAction, Cli};
use crate::config::loader::load_config;
use crate::instance::info::InstanceInfo;
use crate::instance::registry::InstanceRegistry;
use crate::interactive::display::detect_terminal_size;
use crate::web::certs::{CertificateEntry, CertificateStore};

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
    eprintln!("Multiple vrunner instances are running:");
    eprintln!("{}", format_instance_list(&instances));
    eprintln!();
    eprint!("Enter the PID of the instance to use (or Ctrl+C to abort): ");
    eprintln!();

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

/// Handle the `vrunner spawn` subcommand.
/// Discovers a running vrunner instance and sends a spawn request via HTTP API.
pub async fn handle_spawn_command(cli: &Cli, cmd: &str, args: &[String], rows: Option<u16>, cols: Option<u16>) -> Result<()> {
    let registry = InstanceRegistry::new()?;
    let info = resolve_instance(cli, &registry)?;

    let url = instance_url(&info, &None);
    let client = reqwest::Client::new();

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

    tracing::info!(target_pid = info.pid, cmd = cmd, "Spawning command on remote instance");

    let resp = client
        .post(format!("{}/api/commands", url))
        .json(&body)
        .send()
        .await?;

    let status = resp.status();
    let result: serde_json::Value = resp.json().await?;

    if status.is_success() {
        let cmd_pid = result["data"]["pid"].as_u64().unwrap_or(0);
        let cmd_id = result["data"]["id"].as_str().unwrap_or("?");
        println!("Command spawned successfully on instance {} (PID {})", info.pid, info.pid);
        println!("  PID:       {}", cmd_pid);
        println!("  VTTY:      {}/api/commands/{}/vtty/html", url, cmd_id);
    } else {
        let error = result["error"].as_str().unwrap_or("Unknown error");
        eprintln!("Failed to spawn command: {}", error);
        std::process::exit(1);
    }

    Ok(())
}

/// Handle the `vrunner freeze` subcommand.
pub async fn handle_freeze_command(cli: &Cli, pid: u32) -> Result<()> {
    let registry = InstanceRegistry::new()?;
    let info = resolve_instance(cli, &registry)?;
    let url = instance_url(&info, &None);
    let client = reqwest::Client::new();

    // Look up the command ID by PID via the instance's API
    let cmd_id = resolve_pid_to_id(&client, &url, pid).await?;

    let resp = client
        .post(format!("{}/api/commands/{}/freeze", url, cmd_id))
        .send()
        .await?;

    let status = resp.status();
    let result: serde_json::Value = resp.json().await?;

    if status.is_success() {
        println!("Command with PID {} frozen (SIGSTOP) on instance {}", pid, info.pid);
    } else {
        let error = result["error"].as_str().unwrap_or("Unknown error");
        eprintln!("Failed to freeze command: {}", error);
        std::process::exit(1);
    }

    Ok(())
}

/// Handle the `vrunner thaw` subcommand.
pub async fn handle_thaw_command(cli: &Cli, pid: u32) -> Result<()> {
    let registry = InstanceRegistry::new()?;
    let info = resolve_instance(cli, &registry)?;
    let url = instance_url(&info, &None);
    let client = reqwest::Client::new();

    // Look up the command ID by PID via the instance's API
    let cmd_id = resolve_pid_to_id(&client, &url, pid).await?;

    let resp = client
        .post(format!("{}/api/commands/{}/thaw", url, cmd_id))
        .send()
        .await?;

    let status = resp.status();
    let result: serde_json::Value = resp.json().await?;

    if status.is_success() {
        println!("Command with PID {} thawed (SIGCONT) on instance {}", pid, info.pid);
    } else {
        let error = result["error"].as_str().unwrap_or("Unknown error");
        eprintln!("Failed to thaw command: {}", error);
        std::process::exit(1);
    }

    Ok(())
}

/// Handle the `vrunner resize-command` subcommand.
///
/// Resizes the VTTY of a running command by PID or name.
/// Resizes both the in-memory buffer and the child PTY (sends SIGWINCH).
/// If rows/cols are 0 (default), uses the current terminal size.
pub async fn handle_resize_command(_cli: &Cli, target: &str, rows: u16, cols: u16) -> Result<()> {
    let registry = InstanceRegistry::new()?;
    let instances = registry.list_instances();

    if instances.is_empty() {
        anyhow::bail!("No running vrunner instances found. Start one first with: vrunner -- <command>");
    }

    // If rows/cols are 0 (default), detect from the current terminal.
    let (rows, cols) = if rows == 0 || cols == 0 {
        match detect_terminal_size() {
            Some((r, c)) => {
                let r = if rows == 0 { r } else { rows };
                let c = if cols == 0 { c } else { cols };
                (r, c)
            }
            None => {
                let r = if rows == 0 { 24 } else { rows };
                let c = if cols == 0 { 80 } else { cols };
                (r, c)
            }
        }
    } else {
        (rows, cols)
    };

    let client = reqwest::Client::new();

    // Fast path: if target is a pure number, treat as PID.
    if let Ok(pid) = target.parse::<u32>() {
        return handle_resize_by_pid(&client, &instances, pid, rows, cols).await;
    }

    // Collect all commands from all instances.
    let all_commands = collect_all_commands(&client, &instances).await;

    if all_commands.is_empty() {
        anyhow::bail!("No running commands found. Use `vrunner list` to see running commands.");
    }

    // Exact match on name alone or full "name args" string.
    let exact: Vec<_> = all_commands.iter()
        .filter(|(_, _, _, name, full)| name == target || full == target)
        .collect();

    if exact.len() == 1 {
        let (inst_pid, ref cmd_id, cmd_pid, _, _) = exact[0];
        let info = instances.iter().find(|i| i.pid == *inst_pid).unwrap();
        let url = instance_url(info, &None);
        return resize_command_by_id(&client, &url, cmd_id, *cmd_pid, *inst_pid, rows, cols).await;
    }
    if exact.len() > 1 {
        eprintln!("Multiple commands match '{}':", target);
        for (_, _, pid, _name, full) in &exact {
            eprintln!("  PID {} — {}", pid, full);
        }
        anyhow::bail!("Ambiguous target. Use PID to disambiguate.");
    }

    // Prefix match on full string, then on name only (same as stop-command).
    let prefix_full: Vec<_> = all_commands.iter()
        .filter(|(_, _, _, _, full)| full.starts_with(target))
        .collect();
    if prefix_full.len() == 1 {
        let (inst_pid, ref cmd_id, cmd_pid, _, _) = prefix_full[0];
        let info = instances.iter().find(|i| i.pid == *inst_pid).unwrap();
        let url = instance_url(info, &None);
        return resize_command_by_id(&client, &url, cmd_id, *cmd_pid, *inst_pid, rows, cols).await;
    }

    let prefix_name: Vec<_> = all_commands.iter()
        .filter(|(_, _, _, name, _)| name.starts_with(target))
        .collect();
    if prefix_name.len() == 1 {
        let (inst_pid, ref cmd_id, cmd_pid, _, _) = prefix_name[0];
        let info = instances.iter().find(|i| i.pid == *inst_pid).unwrap();
        let url = instance_url(info, &None);
        return resize_command_by_id(&client, &url, cmd_id, *cmd_pid, *inst_pid, rows, cols).await;
    }

    anyhow::bail!("No command matching '{}' found. Use `vrunner list` to see running commands.", target);
}

/// Resize a command by its UUID via the instance's HTTP API.
pub async fn resize_command_by_id(
    client: &reqwest::Client,
    url: &str,
    cmd_id: &str,
    cmd_pid: u32,
    inst_pid: u32,
    rows: u16,
    cols: u16,
) -> Result<()> {
    let resp = client
        .post(format!("{}/api/commands/{}/resize", url, cmd_id))
        .json(&serde_json::json!({ "rows": rows, "cols": cols }))
        .send()
        .await?;

    let status = resp.status();
    let body: serde_json::Value = resp.json().await?;

    if status.is_success() && body.get("status").and_then(|s| s.as_str()) == Some("ok") {
        println!("Resized command with PID {} to {}x{} on instance {} (PID {})", cmd_pid, rows, cols, inst_pid, inst_pid);
        Ok(())
    } else {
        let err_msg = body.get("error").and_then(|e| e.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("HTTP {}", status));
        anyhow::bail!("Failed to resize command with PID {}: {}", cmd_pid, err_msg);
    }
}

/// Resize a command by its OS PID, trying all running instances.
pub async fn handle_resize_by_pid(
    client: &reqwest::Client,
    instances: &[InstanceInfo],
    pid: u32,
    rows: u16,
    cols: u16,
) -> Result<()> {
    for info in instances {
        let url = instance_url(info, &None);
        match resolve_pid_to_id(client, &url, pid).await {
            Ok(cmd_id) => {
                return resize_command_by_id(client, &url, &cmd_id, pid, info.pid, rows, cols).await;
            }
            Err(_) => continue,
        }
    }
    anyhow::bail!("No command found with PID {}. Use `vrunner list` to see running commands.", pid);
}

/// Build the full display string ("name arg1 arg2") from a command JSON value.
fn build_full_display_string(cmd: &serde_json::Value) -> (String, String) {
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

/// Handle the `vrunner list` subcommand.
///
/// Queries running vrunner instances and shows their commands in a
/// two-level indented hierarchy:
///
///   INSTANCE  PID: 12345  PORT: 9090  BIND: 127.0.0.1  DAEMON: no  DISPLAY: yes
///     COMMAND  htop                              PID: 5678  CERT: -
///     COMMAND  vim file.txt                      PID: 5679  CERT: my-app
///
/// When `--target <PID>` is provided, only that instance is listed.
/// Unreachable instances show an `[ERROR]` line under their header.
/// Instances with no commands show `(no commands)`.
pub async fn handle_list_command(cli: &Cli) -> Result<()> {
    let registry = InstanceRegistry::new()?;
    let all_instances = registry.list_instances();

    // Resolve target: filter to a single instance if --target is given.
    let instances: Vec<InstanceInfo> = if let Some(target_pid) = cli.target {
        match all_instances.iter().find(|i| i.pid == target_pid) {
            Some(info) => vec![info.clone()],
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
        all_instances
    };

    if instances.is_empty() {
        println!("No running vrunner instances.");
        return Ok(());
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()?;

    for info in &instances {
        println!("{}", format_instance_header(info));

        let url = instance_url(info, &None);

        match client.get(format!("{}/api/commands", url)).send().await {
            Ok(resp) => {
                match resp.json::<serde_json::Value>().await {
                    Ok(json) => {
                        if json["status"] == "ok" {
                            if let Some(cmds) = json["data"].as_array() {
                                if cmds.is_empty() {
                                    println!("  {}", c("(no commands)", Color::Yellow, false));
                                } else {
                                    for cmd in cmds {
                                        if let Some(line) = format_command(cmd) {
                                            println!("{}", line);
                                        }
                                    }
                                }
                            } else {
                                println!("  {}  Invalid API response: expected array", c("[ERROR]", Color::Red, true));
                            }
                        } else {
                            let err = json["error"].as_str().unwrap_or("unknown error");
                            println!("  {}  API returned error: {}", c("[ERROR]", Color::Red, true), err);
                        }
                    }
                    Err(e) => {
                        println!("  {}  Invalid API response: {}", c("[ERROR]", Color::Red, true), e);
                    }
                }
            }
            Err(e) => {
                println!("  {}  Instance unreachable: {}", c("[ERROR]", Color::Red, true), e);
            }
        }

        // Blank line between instances for readability
        if instances.len() > 1 {
            println!();
        }
    }

    Ok(())
}

/// Format an instance header line for `vrunner list` output.
pub fn format_instance_header(info: &InstanceInfo) -> String {
    let daemon = if info.daemon { "yes" } else { "no" };
    let display = if info.display { "yes" } else { "no" };
    format!(
        "{}  {} {}  {} {}  {} {}  {} {}  {} {}",
        c("INSTANCE", Color::Blue, true),
        c("PID:", Color::DarkGrey, false), info.pid,
        c("PORT:", Color::DarkGrey, false), info.port,
        c("BIND:", Color::DarkGrey, false), info.bind,
        c("DAEMON:", Color::DarkGrey, false), daemon,
        c("DISPLAY:", Color::DarkGrey, false), display,
    )
}

/// Format a single command line for `vrunner list` output.
/// Returns None if the JSON value lacks required fields.
pub fn format_command(cmd: &serde_json::Value) -> Option<String> {
    let name = cmd.get("name")?.as_str()?;
    let args = cmd.get("args")?.as_array()?;
    let args_vec: Vec<&str> = args.iter().filter_map(|v| v.as_str()).collect();
    let display_name = if args_vec.is_empty() {
        name.to_string()
    } else {
        format!("{} {}", name, args_vec.join(" "))
    };
    // Truncate long command names for readability
    let truncated = if display_name.len() > 40 {
        format!("{}...", &display_name[..37])
    } else {
        display_name
    };
    let pid = cmd.get("pid").and_then(|v| v.as_u64()).unwrap_or(0);
    let cert = cmd.get("certificate").and_then(|v| v.as_str()).unwrap_or("-");

    Some(format!(
        "  {} {}  {}",
        c(&format!("{:<10}", pid), Color::Cyan, false),
        c(&format!("{:<20}", truncated), Color::Reset, false),
        c(&format!("CERT: {}", cert), Color::DarkGrey, false),
    ))
}

/// Stop a specific command by PID or name on any running instance.
///
/// If `target` parses as a u32, it is treated as a PID and resolved
/// via `resolve_pid_to_id` (same as freeze/thaw).
///
/// If `target` is a name (or "name args..."), matching proceeds in three
/// rounds with increasing looseness.  A match from an earlier round wins:
///   1. Exact: `name == target` or `name arg1 arg2 ... == target`
///   2. Prefix on full: `name arg1 arg2 ...` starts with `target`
///   3. Prefix on name: `name` starts with `target`
/// If after all rounds exactly one command matches, it is stopped.
/// If multiple commands match, an error lists them and suggests using a
/// PID to disambiguate.
///
/// Returns true if exactly one command was found and stopped.
pub async fn handle_stop_command(_cli: &Cli, target: Option<&str>) -> Result<bool> {
    let registry = InstanceRegistry::new()?;
    let instances = registry.list_instances();

    if instances.is_empty() {
        return Ok(false);
    }

    let client = reqwest::Client::new();

    // If no target given, stop the only command if there is exactly one.
    let target = match target {
        Some(t) => t,
        None => {
            // Collect all commands from all instances
            let all_commands = collect_all_commands(&client, &instances).await;

            match all_commands.len() {
                0 => {
                    eprintln!("No commands running.");
                    return Ok(false);
                }
                1 => {
                    let (inst_pid, ref cmd_id, cmd_pid, _, ref full) = all_commands[0];
                    let info = instances.iter().find(|i| i.pid == inst_pid).unwrap();
                    let url = instance_url(info, &None);
                    eprintln!("Stopping only command: {} (PID {})", full, cmd_pid);
                    return stop_command_by_id(&client, &url, cmd_id, cmd_pid, inst_pid).await;
                }
                _ => {
                    eprintln!("Multiple commands running. Specify which one to stop:");
                    for (_, _, cmd_pid, _, full) in &all_commands {
                        eprintln!("  PID {} — {}", cmd_pid, full);
                    }
                    eprintln!("Usage: vrunner stop-command <PID or name>");
                    return Ok(false);
                }
            }
        }
    };

    // Fast path: if target is a pure number, treat as PID.
    if let Ok(pid) = target.parse::<u32>() {
        return handle_stop_command_by_pid_on_instances(&client, &instances, pid).await;
    }

    // Collect all commands from all instances.
    let all_commands = collect_all_commands(&client, &instances).await;

    if all_commands.is_empty() {
        return Ok(false);
    }

    // Round 1: exact match on name alone or full "name args" string.
    let exact: Vec<_> = all_commands.iter()
        .filter(|(_, _, _, name, full)| name == target || full == target)
        .collect();

    if exact.len() == 1 {
        let (inst_pid, ref cmd_id, cmd_pid, _, _) = exact[0];
        let info = instances.iter().find(|i| i.pid == *inst_pid).unwrap();
        let url = instance_url(info, &None);
        return stop_command_by_id(&client, &url, cmd_id, *cmd_pid, *inst_pid).await;
    }
    if exact.len() > 1 {
        eprintln!("Multiple commands match '{}':", target);
        for (inst_pid, _, cmd_pid, _, full) in &exact {
            eprintln!("  PID {} — {} (on instance {})", cmd_pid, full, inst_pid);
        }
        eprintln!("Use a PID to disambiguate.");
        return Ok(false);
    }

    // Round 2: prefix match on full "name args" string.
    let prefix_full: Vec<_> = all_commands.iter()
        .filter(|(_, _, _, _, full)| full.starts_with(target))
        .collect();

    if prefix_full.len() == 1 {
        let (inst_pid, ref cmd_id, cmd_pid, _, _) = prefix_full[0];
        let info = instances.iter().find(|i| i.pid == *inst_pid).unwrap();
        let url = instance_url(info, &None);
        return stop_command_by_id(&client, &url, cmd_id, *cmd_pid, *inst_pid).await;
    }
    if prefix_full.len() > 1 {
        eprintln!("Multiple commands match '{}':", target);
        for (inst_pid, _, cmd_pid, _, full) in &prefix_full {
            eprintln!("  PID {} — {} (on instance {})", cmd_pid, full, inst_pid);
        }
        eprintln!("Use a longer prefix or a PID to disambiguate.");
        return Ok(false);
    }

    // Round 3: prefix match on name alone.
    let prefix_name: Vec<_> = all_commands.iter()
        .filter(|(_, _, _, name, _)| name.starts_with(target))
        .collect();

    if prefix_name.len() == 1 {
        let (inst_pid, ref cmd_id, cmd_pid, _, _) = prefix_name[0];
        let info = instances.iter().find(|i| i.pid == *inst_pid).unwrap();
        let url = instance_url(info, &None);
        return stop_command_by_id(&client, &url, cmd_id, *cmd_pid, *inst_pid).await;
    }
    if prefix_name.len() > 1 {
        eprintln!("Multiple commands match '{}':", target);
        for (inst_pid, _, cmd_pid, _, full) in &prefix_name {
            eprintln!("  PID {} — {} (on instance {})", cmd_pid, full, inst_pid);
        }
        eprintln!("Use a longer prefix or a PID to disambiguate.");
        return Ok(false);
    }

    // No match at all.
    Ok(false)
}

/// Internal: send the kill request for a resolved command ID.
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
            let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::json!({"status": "unknown"}));
            if status.is_success() && body.get("status").and_then(|s| s.as_str()) == Some("ok") {
                println!("Command with PID {} stopped on instance {} (PID {})", cmd_pid, inst_pid, inst_pid);
                Ok(true)
            } else {
                let err_msg = body.get("error").and_then(|e| e.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| format!("HTTP {}", status));
                eprintln!("Failed to stop command with PID {}: {}", cmd_pid, err_msg);
                Ok(false)
            }
        }
        Err(e) => {
            eprintln!("Failed to stop command with PID {}: {}", cmd_pid, e);
            Ok(false)
        }
    }
}

/// Internal: stop a command by PID on a list of instances.
/// Used by handle_stop_command when target parses as a number.
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
                let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::json!({"status": "unknown"}));
                if status.is_success() && body.get("status").and_then(|s| s.as_str()) == Some("ok") {
                    println!("Command with PID {} stopped on instance {} (PID {})", pid, info.pid, info.pid);
                    return Ok(true);
                } else {
                    let err_msg = body.get("error").and_then(|e| e.as_str())
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| format!("HTTP {}", status));
                    eprintln!("Failed to stop command with PID {}: {}", pid, err_msg);
                    return Ok(false);
                }
            }
            Err(e) => {
                eprintln!("Failed to stop command with PID {}: {}", pid, e);
                return Ok(false);
            }
        }
    }

    Ok(false)
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

/// Handle the `vrunner list-vrunner` subcommand.
///
/// Lists vrunner instances in tab-separated (TSV) format for machine parsing.
pub async fn handle_list_vrunner_command(cli: &Cli) -> Result<()> {
    let registry = InstanceRegistry::new()?;
    let all_instances = registry.list_instances();
    let instances = resolve_targeted_instances(cli, &all_instances)?;

    // Print TSV header
    println!("PID\tPORT\tBIND\tDAEMON\tDISPLAY\tSTARTUP_CMD");
    for info in &instances {
        let startup = info.command.as_deref().unwrap_or("(idle)");
        let daemon = if info.daemon { "yes" } else { "no" };
        let display = if info.display { "yes" } else { "no" };
        println!("{}\t{}\t{}\t{}\t{}\t{}",
            info.pid, info.port, info.bind, daemon, display, startup);
    }
    Ok(())
}

/// Handle the `vrunner list-commands` subcommand.
///
/// Lists all running commands across instances in tab-separated (TSV) format.
pub async fn handle_list_commands_command(cli: &Cli) -> Result<()> {
    let registry = InstanceRegistry::new()?;
    let all_instances = registry.list_instances();
    let instances = resolve_targeted_instances(cli, &all_instances)?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()?;

    // Print TSV header
    println!("VRUNNER_PID\tCMD_PID\tNAME\tARGS\tCERT");

    for info in &instances {
        let url = instance_url(info, &None);
        match client.get(format!("{}/api/commands", url)).send().await {
            Ok(resp) => {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    if let Some(cmds) = json["data"].as_array() {
                        for cmd in cmds {
                            let cmd_pid = cmd.get("pid").and_then(|v| v.as_u64()).unwrap_or(0);
                            let name = cmd.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                            let args = serde_json::to_string(cmd.get("args").unwrap_or(&serde_json::json!([]))).unwrap_or_else(|_| "[]".to_string());
                            let cert = cmd.get("certificate").and_then(|v| v.as_str()).unwrap_or("-");
                            println!("{}\t{}\t{}\t{}\t{}",
                                info.pid, cmd_pid, name, args, cert);
                        }
                    }
                }
            }
            Err(_) => {
                // Skip unreachable instances silently in TSV mode
            }
        }
    }
    Ok(())
}

/// Handle the `vrunner cert` subcommands (generate, list, show, remove).
///
/// These are synchronous operations that don't require the tokio runtime.
pub fn handle_cert_command(action: &CertAction) -> Result<()> {
    match action {
        CertAction::Generate { name } => {
            let mut store = CertificateStore::new();
            let entry = store.generate(name)?;
            let token = entry.derive_token()?;
            println!("Certificate '{}' generated successfully.", name);
            println!("  Certificate: {}", entry.cert_file);
            println!("  Key:        {}", entry.key_file);
            println!("  Token:      {}... (first 16 of 64 chars)", &token[..16]);
        }
        CertAction::List => {
            let cfg = load_config(None)?;
            let entries: Vec<CertificateEntry> = cfg
                .certificates
                .entries
                .iter()
                .map(|e| CertificateEntry {
                    name: e.name.clone(),
                    cert_file: e.cert_file.clone(),
                    key_file: e.key_file.clone(),
                })
                .collect();

            if entries.is_empty() {
                println!("No certificates configured.");
                return Ok(());
            }

            match CertificateStore::load_or_generate(entries) {
                Ok(store) => {
                    let certs = store.list();
                    if certs.is_empty() {
                        println!("No certificates in the store.");
                    } else {
                        println!("{:<25} {:<50} {}", "NAME", "CERT FILE", "TOKEN (prefix)");
                        println!("{}", "-".repeat(100));
                        for cert in certs {
                            let token_preview = cert
                                .derive_token()
                                .map(|t| format!("{}...", &t[..16]))
                                .unwrap_or_else(|_| "<error>".to_string());
                            println!("{:<25} {:<50} {}", cert.name, cert.cert_file, token_preview);
                        }
                    }
                }
                Err(e) => {
                    anyhow::bail!("Failed to load certificates: {}", e);
                }
            }
        }
        CertAction::Show { name } => {
            let cfg = load_config(None)?;
            let entries: Vec<CertificateEntry> = cfg
                .certificates
                .entries
                .iter()
                .map(|e| CertificateEntry {
                    name: e.name.clone(),
                    cert_file: e.cert_file.clone(),
                    key_file: e.key_file.clone(),
                })
                .collect();

            let store = CertificateStore::load_or_generate(entries)?;

            match store.get(name) {
                Some(entry) => {
                    let token = entry.derive_token()?;
                    println!("Certificate: {}", entry.name);
                    println!("  Certificate: {}", entry.cert_file);
                    println!("  Key:        {}", entry.key_file);
                    println!("  Token:      {} (full SHA-256 hex)", token);
                    println!("  Token (16): {}...", &token[..16]);
                }
                None => {
                    anyhow::bail!("Certificate '{}' not found in store", name);
                }
            }
        }
        CertAction::Remove { name } => {
            let cfg = load_config(None)?;
            let entries: Vec<CertificateEntry> = cfg
                .certificates
                .entries
                .iter()
                .map(|e| CertificateEntry {
                    name: e.name.clone(),
                    cert_file: e.cert_file.clone(),
                    key_file: e.key_file.clone(),
                })
                .collect();

            let mut store = CertificateStore::load_or_generate(entries)?;

            match store.remove(name) {
                Some(entry) => {
                    println!("Certificate '{}' removed from store.", name);
                    println!("  Certificate: {}", entry.cert_file);
                    println!("  Key:        {}", entry.key_file);
                    println!("  Note: Files were not deleted.");
                }
                None => {
                    anyhow::bail!("Certificate '{}' not found in store", name);
                }
            }
        }
    }
    Ok(())
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
                    eprintln!("No vrunner instances running.");
                    std::process::exit(1);
                }
                1 => {
                    let p = instances[0].pid;
                    eprintln!("Stopping only running instance (PID {})", p);
                    p
                }
                _ => {
                    eprintln!("Multiple vrunner instances running. Specify which one to stop:");
                    for inst in instances {
                        eprintln!("  PID {} — port {}", inst.pid, inst.port);
                    }
                    eprintln!("Usage: vrunner stop <PID>");
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
