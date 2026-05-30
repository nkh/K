use anyhow::Result;
use crossterm::style::Color;

use crate::cli::args::Cli;
use crate::instance::info::InstanceInfo;
use crate::instance::registry::InstanceRegistry;

use super::common::{c, http_client, instance_url, resolve_targeted_instances};

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
/// Queries running vrunner instances and shows their commands in a
/// two-level indented hierarchy:
///
///   INSTANCE  PID: 12345  PORT: 9090  BIND: 127.0.0.1  DAEMON: no  DISPLAY: yes  UP: 2h 15m
///     COMMAND  htop                              PID: 5678  UP: 45m 12s  SIZE: 80x24  CERT: -
///     COMMAND  vim file.txt                      PID: 5679  UP: 2m 5s   SIZE: 120x40  CERT: my-app
///
/// When `--target <PID>` is provided, only that instance is listed.
/// Unreachable instances show an `[ERROR]` line under their header.
/// Instances with no commands show `(no commands)`.
pub async fn handle_list_command(cli: &Cli) -> Result<()> {
    let registry = InstanceRegistry::new()?;
    let all_instances = registry.list_instances();

    if all_instances.is_empty() {
        println!("No running vrunner instances.");
        return Ok(());
    }

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

    if instances.is_empty() {
        println!("No running vrunner instances.");
        return Ok(());
    }

    let client = http_client();

    // Deduplicate instances by bind:port to prevent showing the same
    // commands twice when two vrunners happen to share the same endpoint.
    let mut seen_endpoints = std::collections::HashSet::new();

    for info in &instances {
        let endpoint = format!("{}:{}", info.bind, info.port);
        if !seen_endpoints.insert(endpoint.clone()) {
            // Duplicate endpoint — skip this instance
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
                                println!("  {}", c("(no commands)", Color::Yellow, false));
                            } else {
                                for cmd in cmds {
                                    // Fetch terminal dimensions for this command
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
    let uptime = info.start_time;
    let uptime_secs = uptime
        .signed_duration_since(chrono::Utc::now())
        .num_seconds()
        .abs() as f64;
    let uptime_str = format_duration(uptime_secs);
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

/// Fetch terminal dimensions for a command by querying the VTTY HTML endpoint.
/// Returns (rows, cols) or None if the fetch fails.
async fn fetch_cmd_dimensions(
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

/// Format a single command line for `vrunner list` output.
/// Returns None if the JSON value lacks required fields.
pub fn format_command(cmd: &serde_json::Value, dims: Option<(usize, usize)>) -> Option<String> {
    let name = cmd.get("name")?.as_str()?;
    let args = cmd.get("args")?.as_array()?;
    let args_vec: Vec<&str> = args.iter().filter_map(|v| v.as_str()).collect();
    let display_name = if args_vec.is_empty() {
        name.to_string()
    } else {
        format!("{} {}", name, args_vec.join(" "))
    };
    // Truncate long command names for readability
    let truncated = if display_name.len() > 30 {
        format!("{}...", &display_name[..27])
    } else {
        display_name
    };
    let pid = cmd.get("pid").and_then(|v| v.as_u64()).unwrap_or(0);
    let cert = cmd
        .get("certificate")
        .and_then(|v| v.as_str())
        .unwrap_or("-");
    let runtime = cmd
        .get("runtime_secs")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
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
        println!(
            "{}\t{}\t{}\t{}\t{}\t{}",
            info.pid, info.port, info.bind, daemon, display, startup
        );
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
    let client = http_client();

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
