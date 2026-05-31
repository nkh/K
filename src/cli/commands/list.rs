use anyhow::Result;
use crossterm::style::Color;

use crate::cli::args::Cli;
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

/// Handle the `vrunner list` subcommand.
///
/// Lists running vrunner instances by reading PID files.
/// Shows instance metadata (PID, daemon, display, uptime, command).
pub fn handle_list_command(cli: &Cli) -> Result<()> {
    let registry = InstanceRegistry::new()?;
    let all_instances = registry.list_instances();

    if all_instances.is_empty() {
        println!("No running vrunner instances.");
        return Ok(());
    }

    // Resolve target: filter to a single instance if --target is given.
    let instances: Vec<_> = if let Some(target_pid) = cli.target {
        match all_instances.iter().find(|i| i.pid == target_pid) {
            Some(info) => vec![info.clone()],
            None => {
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
        all_instances
    };

    for info in &instances {
        println!("{}", format_instance_header(info));

        let command = info.command.as_deref().unwrap_or("(idle)");
        println!(
            "  {} {}",
            c("COMMAND:", Color::DarkGrey, false),
            c(command, Color::Reset, false),
        );

        // Blank line between instances for readability
        if instances.len() > 1 {
            println!();
        }
    }

    Ok(())
}

/// Format an instance header line for `vrunner list` output.
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

/// Format a single command line for `vrunner list` output.
/// Returns None if the JSON value lacks required fields.
pub fn format_command(cmd: &serde_json::Value, _dims: Option<(usize, usize)>) -> Option<String> {
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

    Some(format!(
        "  {} {}",
        c(&format!("{:<10}", pid), Color::Cyan, false),
        c(&format!("{:<20}", truncated), Color::Reset, false),
    ))
}
