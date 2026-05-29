use anyhow::Result;

use crate::cli::args::Cli;
use crate::cli::commands::common::{collect_all_commands, http_client, instance_url, resolve_targeted_instances};
use crate::instance::registry::InstanceRegistry;

/// Handle the `vrunner screenshot [TARGET]` subcommand.
///
/// Fetches the VTTY buffer of the specified (or sole) running command,
/// renders it as a PNG image, and writes it to the output file.
pub async fn handle_screenshot_command(
    cli: &Cli,
    target: Option<&str>,
    output: &str,
    cell_w: u32,
    cell_h: u32,
    scale: u32,
) -> Result<()> {
    let registry = InstanceRegistry::new()?;
    let all_instances = registry.list_instances();
    let instances = resolve_targeted_instances(cli, &all_instances)?;
    let client = http_client();

    let all_commands = collect_all_commands(&client, &instances).await;

    let (instance_pid, cmd_id, _cmd_pid, name, _full) = match target {
        Some(t) => {
            if let Ok(pid) = t.parse::<u32>() {
                match all_commands.iter().find(|(_, _, p, _, _)| *p == pid) {
                    Some(entry) => entry.clone(),
                    None => anyhow::bail!(
                        "No command found with PID {}. Use `vrunner list` to see running commands.",
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

    let url = instance_url(info, &None);

    // Fetch the PNG from the REST API
    let png_url = format!(
        "{}/api/commands/{}/vtty/png?cell_w={}&cell_h={}&scale={}",
        url, cmd_id, cell_w, cell_h, scale
    );
    let resp = client.get(&png_url).send().await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("Failed to fetch screenshot (HTTP {}): {}", status, body);
    }

    let bytes = resp.bytes().await?;
    tokio::fs::write(output, &bytes).await?;

    tracing::info!(
        "Screenshot saved to '{}' ({} bytes, {}x{}px, scale {}x) for command '{}'",
        output,
        bytes.len(),
        cell_w,
        cell_h,
        scale,
        name
    );

    Ok(())
}
