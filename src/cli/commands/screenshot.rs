#![cfg(feature = "vrunner")]
#![allow(dead_code, unused_imports)]
use anyhow::Result;

use crate::cli::args::Cli;
use crate::cli::commands::common::{
    collect_all_commands, http_client, instance_url, resolve_targeted_instances,
};
use crate::cli::commands::list::fetch_cmd_dimensions;
use crate::instance::registry::InstanceRegistry;

/// Handle the `vrunner screenshot [TARGET]` subcommand.
///
/// Fetches the VTTY buffer of the specified (or sole) running command,
/// renders it as a PNG image using a TrueType font, and writes it to
/// the output file.  If no output path is given, generates one from
/// the pattern `vrunner_YYYYMMDD_HHMMSS_rows_cols_command_args.png`.
///
/// The full output path is printed to stdout after the screenshot is
/// saved, so the user can easily locate the file.
pub async fn handle_screenshot_command(
    cli: &Cli,
    target: Option<&str>,
    output: Option<&str>,
    font_size: f32,
    font_name: Option<&str>,
) -> Result<()> {
    let registry = InstanceRegistry::new()?;
    let all_instances = registry.list_instances();
    let instances = resolve_targeted_instances(cli, &all_instances)?;
    let client = http_client();

    let all_commands = collect_all_commands(&client, &instances).await;

    let (instance_pid, cmd_id, _cmd_pid, name, full) = match target {
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

    // Build the PNG endpoint URL with font parameters.
    let mut png_url = format!(
        "{}/api/commands/{}/vtty/png?font_size={}",
        url, cmd_id, font_size
    );
    if let Some(font) = font_name {
        png_url.push_str(&format!(
            "&font_name={}",
            urlencoding::encode(font)
        ));
    }

    let resp = client.get(&png_url).send().await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("Failed to fetch screenshot (HTTP {}): {}", status, body);
    }

    let bytes = resp.bytes().await?;

    // Auto-generate filename if not specified.
    // Format: vrunner_YYYYMMDD_HHMMSS_rows_cols_command_args.png
    // Spaces in command/args are replaced with underscores.
    let output_path = match output {
        Some(p) => p.to_string(),
        None => {
            let ts = chrono::Local::now().format("%Y%m%d_%H%M%S");

            // Try to fetch terminal dimensions for the filename.
            // If the fetch fails, omit dimensions rather than erroring.
            let dims_part = match fetch_cmd_dimensions(&client, &url, &cmd_id).await {
                Some((rows, cols)) => format!("{}_{}", rows, cols),
                None => String::new(),
            };

            // Build the command+args part: replace spaces with underscores,
            // strip non-safe characters.
            let cmd_part: String = full
                .chars()
                .map(|c| {
                    if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' {
                        c
                    } else {
                        '_'
                    }
                })
                .collect();

            // Truncate overly long command+args to keep filename reasonable
            let cmd_truncated = if cmd_part.len() > 120 {
                format!("{}...", &cmd_part[..117])
            } else {
                cmd_part
            };

            match dims_part.as_str() {
                "" => format!("vrunner_{}_{}.png", ts, cmd_truncated),
                dims => format!("vrunner_{}_{}_{}.png", ts, dims, cmd_truncated),
            }
        }
    };

    tokio::fs::write(&output_path, &bytes).await?;

    // Print the full path to stdout so the user can easily find the file.
    // Use absolute path if the output_path is relative.
    let display_path = std::path::Path::new(&output_path);
    let abs_path = if display_path.is_absolute() {
        output_path.clone()
    } else {
        match std::env::current_dir() {
            Ok(cwd) => cwd.join(&output_path).to_string_lossy().to_string(),
            Err(_) => output_path.clone(),
        }
    };
    println!("{}", abs_path);

    tracing::info!(
        "Screenshot saved to '{}' ({} bytes, font_size={}) for command '{}'",
        output_path,
        bytes.len(),
        font_size,
        name
    );

    Ok(())
}
