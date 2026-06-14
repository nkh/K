#![cfg(feature = "vrw")]
#![allow(dead_code, unused_imports)]
use anyhow::Result;

use crate::cli::args::Cli;
use crate::cli::commands::common::{
    build_command_select_items, collect_all_commands, http_client,
    resolve_target_command, resolve_targeted_instances, SelectLabelStyle, VrwClient,
};
use crate::cli::commands::list::fetch_cmd_dimensions;
use crate::instance::registry::InstanceRegistry;

/// Handle the `vrw screenshot [TARGET]` subcommand.
///
/// Fetches the VTTY buffer of the specified (or sole) running command,
/// renders it as a PNG image using a TrueType font, and writes it to
/// the output file.  If no output path is given, generates one from
/// the pattern `vrw_YYYYMMDD_HHMMSS_rows_cols_command_args.png`.
///
/// The full output path is printed to stdout after the screenshot is
/// saved, so the user can easily locate the file.
pub async fn handle_screenshot_command(
    cli: &Cli,
    target: Option<&str>,
    output: Option<&str>,
    font_size: f32,
    font_name: Option<&str>,
    interactive: bool,
) -> Result<()> {
    let registry = InstanceRegistry::new()?;
    let all_instances = registry.list_instances();
    let instances = resolve_targeted_instances(cli, &all_instances)?;
    let client = http_client();

    let all_commands = collect_all_commands(&client, &instances).await;

    // Interactive mode
    if interactive && target.is_none() {
        let items = build_command_select_items(&all_commands, SelectLabelStyle::FullWithPid);
        let selected = crate::cli::interactive_select::select_items(
            &items,
            "Select commands to screenshot [space-separated numbers]",
        )?;
        for item in &selected {
            let (inst_pid, cmd_id, _, _, full) = all_commands
                .iter()
                .find(|(_, id, _, _, _)| id == &item.id)
                .expect("selected item must exist");
            let info = instances
                .iter()
                .find(|i| i.pid == *inst_pid)
                .expect("instance must exist");
            let vrw = VrwClient::new(client.clone(), info);
            let bytes = fetch_screenshot_bytes(&vrw, &cmd_id, font_size, font_name).await?;
            let path =
                generate_output_path(output, &vrw, &cmd_id, &full, font_size, font_name).await;
            tokio::fs::write(&path, &bytes).await?;
            println!("{}", resolve_abs_path(&path));
        }
        return Ok(());
    }

    let (inst_pid, cmd_id, _, name, full) =
        resolve_target_command(target, &all_commands, "No command")?;
    let info = instances
        .iter()
        .find(|i| i.pid == inst_pid)
        .expect("instance must exist");
    let vrw = VrwClient::new(client, info);

    let bytes = fetch_screenshot_bytes(&vrw, &cmd_id, font_size, font_name).await?;
    let path = generate_output_path(output, &vrw, &cmd_id, &full, font_size, font_name).await;
    tokio::fs::write(&path, &bytes).await?;
    println!("{}", resolve_abs_path(&path));

    tracing::info!(
        "Screenshot saved to '{}' ({} bytes) for command '{}'",
        path,
        bytes.len(),
        name
    );
    Ok(())
}

/// Fetch screenshot PNG bytes from the API.
async fn fetch_screenshot_bytes(
    client: &VrwClient,
    cmd_id: &str,
    font_size: f32,
    font_name: Option<&str>,
) -> Result<bytes::Bytes> {
    let mut url = format!(
        "/api/commands/{}/vtty/png?font_size={}",
        cmd_id, font_size
    );
    if let Some(font) = font_name {
        url.push_str(&format!("&font_name={}", urlencoding::encode(font)));
    }
    client.get_bytes(&url).await
}

/// Sanitize a string for use as a filename component.
fn sanitize_filename(s: &str) -> String {
    let sanitized: String = s
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if sanitized.len() > 120 {
        format!("{}...", &sanitized[..117])
    } else {
        sanitized
    }
}

/// Generate the output file path for a screenshot.
async fn generate_output_path(
    output: Option<&str>,
    client: &VrwClient,
    cmd_id: &str,
    full: &str,
    _font_size: f32,
    _font_name: Option<&str>,
) -> String {
    match output {
        Some(p) => p.to_string(),
        None => {
            let ts = chrono::Local::now().format("%Y%m%d_%H%M%S");
            let dims_part =
                match fetch_cmd_dimensions(&http_client(), client.base_url(), cmd_id).await {
                    Some((rows, cols)) => format!("{}_{}", rows, cols),
                    None => String::new(),
                };
            let cmd_part = sanitize_filename(full);
            match dims_part.as_str() {
                "" => format!("vrw_{}_{}.png", ts, cmd_part),
                dims => format!("vrw_{}_{}_{}.png", ts, dims, cmd_part),
            }
        }
    }
}

/// Resolve a path to an absolute path for display.
fn resolve_abs_path(path: &str) -> String {
    let p = std::path::Path::new(path);
    if p.is_absolute() {
        path.to_string()
    } else if let Ok(cwd) = std::env::current_dir() {
        cwd.join(path).to_string_lossy().to_string()
    } else {
        path.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test filename sanitization logic used in screenshot.
    #[test]
    fn test_screenshot_filename_sanitization() {
        let full = "cargo run --release 2>&1";
        let cmd_part = sanitize_filename(full);
        assert!(!cmd_part.contains(' '), "spaces replaced");
        assert!(!cmd_part.contains('&'), "special chars replaced");
        assert!(cmd_part.contains("cargo"), "alphanumeric preserved");
        assert!(cmd_part.contains("-"), "hyphens preserved");

        // Truncation at 120 chars
        let long_name = "a".repeat(200);
        let truncated = sanitize_filename(&long_name);
        assert!(truncated.len() <= 123, "truncated + ellipsis <= 123 chars");
        assert!(truncated.ends_with("..."), "truncated ends with ellipsis");
    }
}
