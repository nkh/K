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
use super::common::{http_client, instance_url, resolve_targeted_instances};

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

    let client = http_client();

    let mut seen_endpoints = std::collections::HashSet::new();

    for info in &instances {
        let endpoint = format!("{}:{}", info.bind, info.port);
        if !seen_endpoints.insert(endpoint.clone()) {
            continue;
        }

        println!("{}", format_instance_header(info));

        let url = instance_url(info, &None);

        match client.get(format!("{}/api/commands", url)).send().await {
            Ok(resp) => match resp.json::<serde_json::Value>().await {
                Ok(json) => {
                    if json["status"] == "ok" {
                        if let Some(cmds) = json["data"].as_array() {
                            if cmds.is_empty() {
                                println!(
                                    "  {}",
                                    c("(no commands)", Color::Yellow, false)
                                );
                            } else {
                                for cmd in cmds {
                                    let cmd_id = cmd.get("id").and_then(|v| v.as_str()).unwrap_or("");
                                    let dims = if !cmd_id.is_empty() {
                                        fetch_cmd_dimensions(&client, &url, cmd_id).await
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
                    } else {
                        let err = json["error"].as_str().unwrap_or("unknown error");
                        println!(
                            "  {}  API returned error: {}",
                            c("[ERROR]", Color::Red, true),
                            err
                        );
                    }
                }
                Err(e) => {
                    println!(
                        "  {}  Invalid API response: {}",
                        c("[ERROR]", Color::Red, true),
                        e
                    );
                }
            },
            Err(e) => {
                println!(
                    "  {}  Instance unreachable: {}",
                    c("[ERROR]", Color::Red, true),
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

/// Fetch terminal dimensions for a command by querying the VTTY HTML endpoint.
#[cfg(feature = "vrw")]
pub async fn fetch_cmd_dimensions(
    client: &reqwest::Client,
    base_url: &str,
    cmd_id: &str,
) -> Option<(usize, usize)> {
    let url = format!("{}/api/commands/{}/vtty/html", base_url, cmd_id);
    let resp = client.get(&url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let json: serde_json::Value = resp.json().await.ok()?;
    let dims = json.get("data")?.get("dimensions")?;
    let rows = dims.get("rows")?.as_u64()? as usize;
    let cols = dims.get("cols")?.as_u64()? as usize;
    Some((rows, cols))
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
    let client = http_client();

    println!("VRW_PID\tCMD_PID\tNAME\tARGS\tCERT");

    for info in &instances {
        let url = instance_url(info, &None);
        match client.get(format!("{}/api/commands", url)).send().await {
            Ok(resp) => {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    if let Some(cmds) = json["data"].as_array() {
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
            Err(_) => {
                // Skip unreachable instances silently in TSV mode
            }
        }
    }
    Ok(())
}
