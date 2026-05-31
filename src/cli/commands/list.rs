use anyhow::Result;
use crossterm::style::Color;

use crate::cli::args::Cli;
use crate::instance::registry::InstanceRegistry;
use crate::ipc::client::send_command;
use crate::ipc::protocol::{ControlCommand, ControlResponse};

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

/// Handle the `vrl list` subcommand.
///
/// Discovers running vrl instances from PID files, then queries each
/// instance via UDS to get its live command list (including commands added
/// via spawn-in).
pub async fn handle_list_command(cli: &Cli) -> Result<()> {
    let registry = InstanceRegistry::new()?;
    let all_instances = registry.list_instances();

    if all_instances.is_empty() {
        println!("No running vrl instances.");
        return Ok(());
    }

    // Resolve target: filter to a single instance if --target is given.
    let instances: Vec<_> = if let Some(target_pid) = cli.target {
        match all_instances.iter().find(|i| i.pid == target_pid) {
            Some(info) => vec![info.clone()],
            None => {
                anyhow::bail!(
                    "No vrl instance found with PID {}. Running instances:\n{}",
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
        all_instances
    };

    for info in &instances {
        println!("{}", format_instance_header(info));

        // Query the instance via UDS for its live command list.
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

        // Blank line between instances for readability
        if instances.len() > 1 {
            println!();
        }
    }

    Ok(())
}

/// Format an instance header line for `vrl list` output.
pub fn format_instance_header(info: &crate::instance::info::InstanceInfo) -> String {
    let daemon = if info.daemon { "yes" } else { "no" };
    let display = if info.display { "yes" } else { "no" };
    let uptime_secs = info
        .start_time
        .signed_duration_since(chrono::Utc::now())
        .num_seconds()
        .abs() as f64;
    let uptime_str = format_duration(uptime_secs);
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

/// Format a single command line for `vrl list` output.
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
    let truncated = if display_name.len() > 30 {
        format!("{}...", &display_name[..27])
    } else {
        display_name
    };
    let pid = cmd.get("pid").and_then(|v| v.as_u64()).unwrap_or(0);
    let status = cmd.get("status").and_then(|v| v.as_str()).unwrap_or("?");
    let runtime = cmd
        .get("runtime_secs")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
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
