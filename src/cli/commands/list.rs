use anyhow::Result;
use crossterm::style::Color;

use crate::cli::args::{Cli, Commands};
use crate::instance::registry::InstanceRegistry;

use super::common::c;

/// Format a duration in seconds as a human-readable string.
fn format_duration(secs: f64) -> String {
    if secs < 60.0 {
        format!("{:.0}s", secs)
    } else if secs < 3600.0 {
        format!("{}m {:.0}s", (secs / 60.0) as u64, secs % 60.0)
    } else {
        format!(
            "{}h {}m",
            (secs / 3600.0) as u64,
            ((secs % 3600.0) / 60.0) as u64
        )
    }
}

// ── vrc (UDS-based) implementation ──

#[cfg(not(feature = "vrw"))]
use crate::ipc::client::send_command;
#[cfg(not(feature = "vrw"))]
use crate::ipc::protocol::{ControlCommand, ControlResponse};

/// Handle the `vrc list` subcommand.
/// Discovers running instances from PID files, queries each via UDS.
#[cfg(not(feature = "vrw"))]
pub async fn handle_list_command(cli: &Cli) -> Result<()> {
    let registry = InstanceRegistry::new()?;
    let all_instances = registry.list_instances();

    if all_instances.is_empty() {
        println!("No running vrc instances.");
        return Ok(());
    }

    let instances: Vec<_> = if let Some(target_pid) = cli.pid {
        match all_instances.iter().find(|i| i.pid == target_pid) {
            Some(info) => vec![info.clone()],
            None => {
                anyhow::bail!(
                    "No vrc instance found with PID {}. Running instances:\n{}",
                    target_pid,
                    all_instances
                        .iter()
                        .map(|i| format!("  PID: {}", i.pid))
                        .collect::<Vec<_>>()
                        .join("\n")
                );
            }
        }
    } else if matches!(&cli.command, Some(Commands::List { interactive: true })) {
        let items: Vec<_> = all_instances
            .iter()
            .map(|i| crate::cli::interactive_select::SelectItem {
                label: format!("PID {}", i.pid),
                id: i.pid.to_string(),
            })
            .collect();
        let selected = crate::cli::interactive_select::select_items(
            &items,
            "Select instances to list [space-separated numbers]",
        )?;
        let selected_pids: std::collections::HashSet<u32> = selected
            .iter()
            .map(|s| s.id.parse::<u32>().unwrap())
            .collect();
        all_instances
            .into_iter()
            .filter(|i| selected_pids.contains(&i.pid))
            .collect()
    } else {
        all_instances
    };

    for info in &instances {
        println!("{}", format_instance_header(info));

        match send_command(info.pid, ControlCommand::List).await {
            Ok(ControlResponse::Ok { data }) => {
                let commands = data.get("commands").and_then(|v| v.as_array());
                match commands {
                    Some(cmds) if !cmds.is_empty() => {
                        for cmd in cmds {
                            if let Some(line) = format_command(cmd) {
                                println!("{line}");
                            }
                        }
                    }
                    _ => {
                        println!(
                            "  {} (no commands)",
                            c("(idle)", Color::DarkGrey, false)
                        );
                    }
                }
            }
            Ok(ControlResponse::Error { error }) => {
                eprintln!(
                    "  {} (could not query commands via UDS: {})",
                    c("WARNING:", Color::Yellow, true),
                    error
                );
            }
            Err(e) => {
                eprintln!(
                    "  {} (UDS error: {})",
                    c("WARNING:", Color::Yellow, true),
                    e
                );
            }
        }

        if instances.len() > 1 {
            println!();
        }
    }

    Ok(())
}

// ── vrw (HTTP-based) implementation ──

#[cfg(feature = "vrw")]
use super::common::{http_client, resolve_targeted_instances, VrwClient};

/// Handle the `vrw list` subcommand.
/// Queries running instances via HTTP API.
#[cfg(feature = "vrw")]
pub async fn handle_list_command(cli: &Cli) -> Result<()> {
    let registry = InstanceRegistry::new()?;
    let all_instances = registry.list_instances();

    if all_instances.is_empty() {
        println!("No running vrw instances.");
        return Ok(());
    }

    let is_interactive = matches!(&cli.command, Some(Commands::List { interactive: true }));
    let instances: Vec<crate::instance::info::InstanceInfo> = if is_interactive && cli.pid.is_none() {
        let items: Vec<_> = all_instances
            .iter()
            .map(|i| crate::cli::interactive_select::SelectItem {
                label: format!("PID {} — port {}", i.pid, i.port),
                id: i.pid.to_string(),
            })
            .collect();
        let selected = crate::cli::interactive_select::select_items(
            &items,
            "Select instances to list [space-separated numbers]",
        )?;
        let selected_pids: std::collections::HashSet<u32> = selected
            .iter()
            .map(|s| s.id.parse::<u32>().unwrap())
            .collect();
        all_instances
            .into_iter()
            .filter(|i| selected_pids.contains(&i.pid))
            .collect()
    } else {
        resolve_targeted_instances(cli, &all_instances)?
    };

    if instances.is_empty() {
        println!("No running vrw instances.");
        return Ok(());
    }

    let mut seen_endpoints = std::collections::HashSet::new();

    for info in &instances {
        let endpoint = format!("{}:{}", info.bind, info.port);
        if !seen_endpoints.insert(endpoint.clone()) {
            continue;
        }

        println!("{}", format_instance_header(info));

        let vrw = VrwClient::new(http_client(), info);

        match vrw.try_get_data("/api/commands").await {
            Some(data) => {
                if let Some(cmds) = data.as_array() {
                    if cmds.is_empty() {
                        println!("  {}", c("(no commands)", Color::Yellow, false));
                    } else {
                        for cmd in cmds {
                            let cmd_id = cmd.get("id").and_then(|v| v.as_str()).unwrap_or("");
                            let dims = if !cmd_id.is_empty() {
                                vrw.try_get_data(&format!("/api/commands/{}/vtty/html", cmd_id)).await
                                    .and_then(|d| {
                                        let dims = d.get("dimensions")?;
                                        let rows = dims.get("rows")?.as_u64()? as usize;
                                        let cols = dims.get("cols")?.as_u64()? as usize;
                                        Some((rows, cols))
                                    })
                            } else {
                                None
                            };
                            if let Some(line) = format_command(cmd, dims) {
                                println!("{}", line);
                            }
                        }
                    }
                } else {
                    println!(
                        "  {}  Invalid API response: expected array",
                        c("[ERROR]", Color::Red, true)
                    );
                }
            }
            None => {
                println!(
                    "  {}  Instance unreachable",
                    c("[ERROR]", Color::Red, true)
                );
            }
        }

        if instances.len() > 1 {
            println!();
        }
    }

    Ok(())
}



// ── Shared formatting functions ──

/// Build a display string from command JSON ("name arg1 arg2") and truncate.
///
/// Returns `(truncated_display, pid, runtime_secs)`.
fn extract_command_display(cmd: &serde_json::Value) -> Option<(String, u64, f64)> {
    let name = cmd.get("name")?.as_str()?;
    let args = cmd.get("args")?.as_array()?;
    let args_vec: Vec<&str> = args.iter().filter_map(|v| v.as_str()).collect();
    let display_name = if args_vec.is_empty() {
        name.to_string()
    } else {
        format!("{} {}", name, args_vec.join(" "))
    };
    let truncated = if display_name.len() > 30 {
        format!("{}...", &display_name[..27])
    } else {
        display_name
    };
    let pid = cmd.get("pid").and_then(|v| v.as_u64()).unwrap_or(0);
    let runtime = cmd
        .get("runtime_secs")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    Some((truncated, pid, runtime))
}

/// Format an instance header line.
pub fn format_instance_header(info: &crate::instance::info::InstanceInfo) -> String {
    let daemon = if info.daemon { "yes" } else { "no" };
    let display = if info.display { "yes" } else { "no" };
    let uptime_secs = info
        .start_time
        .signed_duration_since(chrono::Utc::now())
        .num_seconds()
        .abs() as f64;
    let uptime_str = format_duration(uptime_secs);

    // vrw adds PORT and BIND columns; vrc omits them.
    #[cfg(feature = "vrw")]
    {
        format!(
            "{}  {} {}  {} {}  {} {}  {} {}  {} {}  {} {}",
            c("INSTANCE", Color::Blue, true),
            c("PID:", Color::DarkGrey, false),
            info.pid,
            c("PORT:", Color::DarkGrey, false),
            info.port,
            c("BIND:", Color::DarkGrey, false),
            info.bind,
            c("DAEMON:", Color::DarkGrey, false),
            daemon,
            c("DISPLAY:", Color::DarkGrey, false),
            display,
            c("UP:", Color::DarkGrey, false),
            c(&uptime_str, Color::Green, false),
        )
    }

    #[cfg(not(feature = "vrw"))]
    {
        format!(
            "{}  {} {}  {} {}  {} {}  {} {}",
            c("INSTANCE", Color::Blue, true),
            c("PID:", Color::DarkGrey, false),
            info.pid,
            c("DAEMON:", Color::DarkGrey, false),
            daemon,
            c("DISPLAY:", Color::DarkGrey, false),
            display,
            c("UP:", Color::DarkGrey, false),
            c(&uptime_str, Color::Green, false),
        )
    }
}

/// Format a single command line (vrc version — with status).
#[cfg(not(feature = "vrw"))]
pub fn format_command(cmd: &serde_json::Value) -> Option<String> {
    let (truncated, pid, runtime) = extract_command_display(cmd)?;
    let status = cmd.get("status").and_then(|v| v.as_str()).unwrap_or("?");
    let runtime_str = format_duration(runtime);

    let status_str = match status {
        "running" => c(&format!("running  {}", runtime_str), Color::Green, false),
        "frozen" => c(&format!("frozen   {}", runtime_str), Color::Yellow, true),
        "exited" => {
            let exit = cmd
                .get("exit_code")
                .and_then(|v| v.as_i64())
                .map(|c| format!("exited {}  {}", c, runtime_str))
                .unwrap_or_else(|| format!("exited ?  {}", runtime_str));
            c(&exit, Color::DarkGrey, false)
        }
        other => c(other, Color::DarkGrey, false),
    };

    Some(format!(
        "  {} {}  {} {}",
        c(&format!("{:<10}", pid), Color::Cyan, false),
        c(&format!("{:<30}", truncated), Color::Reset, false),
        c("STATUS:", Color::DarkGrey, false),
        status_str,
    ))
}

/// Format a single command line (vrw version — with dims and cert).
#[cfg(feature = "vrw")]
pub fn format_command(cmd: &serde_json::Value, dims: Option<(usize, usize)>) -> Option<String> {
    let (truncated, pid, runtime) = extract_command_display(cmd)?;
    let cert = cmd
        .get("certificate")
        .and_then(|v| v.as_str())
        .unwrap_or("-");
    let runtime_str = format_duration(runtime);
    let dims_str = match dims {
        Some((r, c)) => format!("{}x{}", r, c),
        None => "-".to_string(),
    };

    Some(format!(
        "  {} {}  {} {}  {} {}  {}",
        c(&format!("{:<10}", pid), Color::Cyan, false),
        c(&format!("{:<20}", truncated), Color::Reset, false),
        c("UP:", Color::DarkGrey, false),
        c(&format!("{:<10}", runtime_str), Color::Green, false),
        c("SIZE:", Color::DarkGrey, false),
        c(&format!("{:<8}", dims_str), Color::Yellow, false),
        c(&format!("CERT: {}", cert), Color::DarkGrey, false),
    ))
}

// ── vrw-only list subcommands ──

/// Handle the `vrw list-vrw` subcommand (TSV format).
#[cfg(feature = "vrw")]
pub async fn handle_list_vrw_command(cli: &Cli) -> Result<()> {
    let registry = InstanceRegistry::new()?;
    let all_instances = registry.list_instances();
    let instances = super::common::resolve_targeted_instances(cli, &all_instances)?;

    println!("PID\tPORT\tBIND\tDAEMON\tDISPLAY\tSTARTUP_CMD");
    for info in &instances {
        let startup = info.command.as_deref().unwrap_or("(idle)");
        let daemon = if info.daemon { "yes" } else { "no" };
        let display = if info.display { "yes" } else { "no" };
        println!(
            "{}\t{}\t{}\t{}\t{}\t{}",
            info.pid, info.port, info.bind, daemon, display, startup
        );
    }
    Ok(())
}

/// Handle the `vrw list-commands` subcommand (TSV format).
#[cfg(feature = "vrw")]
pub async fn handle_list_commands_command(cli: &Cli) -> Result<()> {
    let registry = InstanceRegistry::new()?;
    let all_instances = registry.list_instances();
    let instances = super::common::resolve_targeted_instances(cli, &all_instances)?;
    println!("VRW_PID\tCMD_PID\tNAME\tARGS\tCERT");

    for info in &instances {
        let vrw = VrwClient::new(http_client(), info);
        if let Some(data) = vrw.try_get_data("/api/commands").await {
            if let Some(cmds) = data.as_array() {
                for cmd in cmds {
                    let cmd_pid = cmd.get("pid").and_then(|v| v.as_u64()).unwrap_or(0);
                    let name = cmd.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                    let args = serde_json::to_string(
                        cmd.get("args").unwrap_or(&serde_json::json!([])),
                    )
                    .unwrap_or_else(|_| "[]".to_string());
                    let cert = cmd
                        .get("certificate")
                        .and_then(|v| v.as_str())
                        .unwrap_or("-");
                    println!("{}\t{}\t{}\t{}\t{}", info.pid, cmd_pid, name, args, cert);
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_duration_seconds() {
        assert_eq!(format_duration(0.0), "0s");
        assert_eq!(format_duration(5.0), "5s");
        assert_eq!(format_duration(59.9), "60s");
    }

    #[test]
    fn test_format_duration_minutes() {
        assert_eq!(format_duration(60.0), "1m 0s");
        assert_eq!(format_duration(90.0), "1m 30s");
        assert_eq!(format_duration(3599.0), "59m 59s");
    }

    #[test]
    fn test_format_duration_hours() {
        assert_eq!(format_duration(3600.0), "1h 0m");
        assert_eq!(format_duration(3661.0), "1h 1m");
        assert_eq!(format_duration(7325.0), "2h 2m");
    }

    #[test]
    fn test_extract_command_display() {
        let cmd = serde_json::json!({
            "name": "bash",
            "args": ["-l", "-c", "echo hello"],
            "pid": 12345,
            "runtime_secs": 65.5
        });
        let result = extract_command_display(&cmd);
        assert!(result.is_some());
        let (display, pid, runtime) = result.unwrap();
        assert_eq!(display, "bash -l -c echo hello");
        assert_eq!(pid, 12345);
        assert_eq!(runtime, 65.5);
    }

    #[test]
    fn test_extract_command_display_no_args() {
        let cmd = serde_json::json!({
            "name": "htop",
            "args": [],
            "pid": 42,
            "runtime_secs": 10.0
        });
        let result = extract_command_display(&cmd);
        assert!(result.is_some());
        let (display, pid, runtime) = result.unwrap();
        assert_eq!(display, "htop");
        assert_eq!(pid, 42);
        assert_eq!(runtime, 10.0);
    }

    #[test]
    fn test_extract_command_display_truncation() {
        let long_name = "a".repeat(35);
        let cmd = serde_json::json!({
            "name": &long_name,
            "args": [],
            "pid": 1,
            "runtime_secs": 1.0
        });
        let result = extract_command_display(&cmd);
        assert!(result.is_some());
        let (display, _, _) = result.unwrap();
        assert!(display.len() <= 33);
        assert!(display.ends_with("..."));
    }

    #[test]
    fn test_extract_command_display_missing_fields() {
        let cmd = serde_json::json!({"name": "test"});
        let result = extract_command_display(&cmd);
        assert!(result.is_none());
    }

    #[test]
    fn test_format_instance_header() {
        let info = crate::instance::info::InstanceInfo {
            pid: 12345,
            #[cfg(feature = "vrw")]
            port: 9090,
            #[cfg(feature = "vrw")]
            bind: "127.0.0.1".to_string(),
            #[cfg(feature = "vrw")]
            name: None,
            start_time: chrono::Utc::now(),
            daemon: false,
            display: true,
            #[cfg(feature = "vrw")]
            command: None,
        };
        let header = format_instance_header(&info);
        assert!(header.contains("INSTANCE"));
        assert!(header.contains("12345"));
        assert!(header.contains("PID:"));
        assert!(header.contains("UP:"));
    }
}
