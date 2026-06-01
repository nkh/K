#![cfg(feature = "vrw")]
#![allow(dead_code, unused_imports)]
use anyhow::Result;

use crate::cli::args::Cli;
use crate::cli::commands::common::{collect_all_commands, http_client, instance_url, resolve_targeted_instances};
use crate::instance::registry::InstanceRegistry;

/// Handle the `vrw cat [TARGET]` subcommand.
///
/// Fetches the VTTY buffer of the specified (or sole) running command
/// and prints it to stdout.  When `color_always` is true the output
/// includes ANSI escape sequences so the terminal renders colours;
/// otherwise plain text (no formatting) is printed.
pub async fn handle_cat_command(cli: &Cli, target: Option<&str>, color_always: bool, interactive: bool) -> Result<()> {
    let registry = InstanceRegistry::new()?;
    let all_instances = registry.list_instances();
    let instances = resolve_targeted_instances(cli, &all_instances)?;
    let client = http_client();

    let all_commands = collect_all_commands(&client, &instances).await;

    // Interactive mode: list commands and let user select
    if interactive && target.is_none() {
        let items: Vec<_> = all_commands
            .iter()
            .map(|(_, id, pid, _name, full)| crate::cli::interactive_select::SelectItem {
                label: format!("{} (PID {})", full, pid),
                id: id.clone(),
            })
            .collect();
        let selected = crate::cli::interactive_select::select_items(
            &items, "Select commands to cat [space-separated numbers]",
        )?;
        for item in &selected {
            cat_by_id(&client, &instances, &all_commands, &item.id, color_always).await?;
        }
        return Ok(());
    }

    let (_, cmd_id, _, _, _) = match target {
        Some(t) => {
            if let Ok(pid) = t.parse::<u32>() {
                match all_commands.iter().find(|(_, _, p, _, _)| *p == pid) {
                    Some(entry) => entry.clone(),
                    None => anyhow::bail!(
                        "No command found with PID {}. Use `vrw list` to see running commands.",
                        pid
                    ),
                }
            } else {
                let matches: Vec<_> = all_commands
                    .iter()
                    .filter(|(_, _, _, n, _)| n.eq_ignore_ascii_case(t))
                    .collect();
                match matches.len() {
                    0 => anyhow::bail!(
                        "No command found matching '{}'. Use `vrw list` to see running commands.",
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
            0 => anyhow::bail!("No running commands. Use `vrw list` to see commands."),
            1 => {
                let cmd_id = all_commands[0].1.clone();
                return cat_by_id(&client, &instances, &all_commands, &cmd_id, color_always).await;
            }
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

    cat_by_id(&client, &instances, &all_commands, &cmd_id, color_always).await
}

/// Cat a single command by its ID.
async fn cat_by_id(
    client: &reqwest::Client,
    instances: &[crate::instance::info::InstanceInfo],
    all_commands: &[(u32, String, u32, String, String)],
    cmd_id: &str,
    color_always: bool,
) -> Result<()> {
    let (instance_pid, _, _, _, _) = all_commands
        .iter()
        .find(|(_, id, _, _, _)| id == cmd_id)
        .expect("command must exist");

    let info = instances
        .iter()
        .find(|i| i.pid == *instance_pid)
        .expect("instance must exist");

    let url = instance_url(info, &None);

    if color_always {
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
        print!("\x1b[0m");
    } else {
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
