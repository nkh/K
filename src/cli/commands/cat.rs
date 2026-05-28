use anyhow::Result;

use crate::cli::args::Cli;
use crate::cli::commands::common::{collect_all_commands, http_client, resolve_targeted_instances};
use crate::instance::registry::InstanceRegistry;

/// Handle the `vrunner cat [TARGET]` subcommand.
///
/// Fetches the VTTY buffer of the specified (or sole) running command
/// and prints it to stdout.  When `color_always` is true the output
/// includes ANSI escape sequences so the terminal renders colours;
/// otherwise plain text (no formatting) is printed.
pub async fn handle_cat_command(cli: &Cli, target: Option<&str>, color_always: bool) -> Result<()> {
    let registry = InstanceRegistry::new()?;
    let all_instances = registry.list_instances();
    let instances = resolve_targeted_instances(cli, &all_instances)?;
    let client = http_client();

    let all_commands = collect_all_commands(&client, &instances).await;

    let (instance_pid, cmd_id, _cmd_pid, _name, _full) = match target {
        Some(t) => {
            // Try numeric PID first
            if let Ok(pid) = t.parse::<u32>() {
                match all_commands.iter().find(|(_, _, p, _, _)| *p == pid) {
                    Some(entry) => entry.clone(),
                    None => anyhow::bail!(
                        "No command found with PID {}. Use `vrunner list` to see running commands.",
                        pid
                    ),
                }
            } else {
                // Match by name
                let matches: Vec<_> = all_commands
                    .iter()
                    .filter(|(_, _, _, n, _)| n.eq_ignore_ascii_case(t))
                    .collect();
                match matches.len() {
                    0 => anyhow::bail!(
                        "No command found matching '{}'. Use `vrunner list` to see running commands.",
                        t
                    ),
                    1 => matches[0].clone(),
                    _ => {
                        let list: Vec<_> = matches.iter().map(|e| format!("  pid {}", e.2)).collect();
                        anyhow::bail!(
                            "Multiple commands matching '{}':\n{}\nUse a PID to disambiguate.",
                            t,
                            list.join("\n")
                        )
                    }
                }
            }
        }
        None => match all_commands.len() {
            0 => anyhow::bail!("No running commands. Use `vrunner list` to see commands."),
            1 => all_commands.into_iter().next().unwrap(),
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
    };

    let info = instances
        .iter()
        .find(|i| i.pid == instance_pid)
        .expect("instance must exist");

    let url = crate::cli::commands::common::instance_url(info, &None);

    if color_always {
        // Fetch the buffer with ANSI escape sequences preserved.
        let resp = client
            .get(format!("{}/api/commands/{}/vtty", url, cmd_id))
            .send()
            .await?;
        let json: serde_json::Value = resp.json().await?;
        if json["status"] != "ok" {
            let err = json["error"].as_str().unwrap_or("unknown error");
            anyhow::bail!("Failed to fetch VTTY buffer: {}", err);
        }
        let content = json["data"]["content"].as_str().unwrap_or("");
        print!("{}", content);
    } else {
        // Plain text — no formatting.
        let resp = client
            .get(format!("{}/api/commands/{}/vtty/text", url, cmd_id))
            .send()
            .await?;
        let json: serde_json::Value = resp.json().await?;
        if json["status"] != "ok" {
            let err = json["error"].as_str().unwrap_or("unknown error");
            anyhow::bail!("Failed to fetch VTTY buffer: {}", err);
        }
        let text = json["data"]["text"].as_str().unwrap_or("");
        print!("{}", text);
    }

    Ok(())
}
