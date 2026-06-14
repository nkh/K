#![cfg(feature = "vrw")]
#![allow(dead_code, unused_imports)]
use anyhow::Result;

use crate::cli::args::Cli;
use crate::cli::commands::common::{
    build_command_select_items, collect_all_commands, http_client,
    resolve_target_command, resolve_targeted_instances, SelectLabelStyle, VrwClient,
};
use crate::instance::registry::InstanceRegistry;

/// Handle the `vrw cat [TARGET]` subcommand.
///
/// Fetches the VTTY buffer of the specified (or sole) running command
/// and prints it to stdout.  When `color_always` is true the output
/// includes ANSI escape sequences so the terminal renders colours;
/// otherwise plain text (no formatting) is printed.
pub async fn handle_cat_command(
    cli: &Cli,
    target: Option<&str>,
    color_always: bool,
    interactive: bool,
) -> Result<()> {
    let registry = InstanceRegistry::new()?;
    let all_instances = registry.list_instances();
    let instances = resolve_targeted_instances(cli, &all_instances)?;
    let client = http_client();
    let all_commands = collect_all_commands(&client, &instances).await;

    // Interactive mode: list commands and let user select
    if interactive && target.is_none() {
        let items = build_command_select_items(&all_commands, SelectLabelStyle::FullWithPid);
        let selected = crate::cli::interactive_select::select_items(
            &items,
            "Select commands to cat [space-separated numbers]",
        )?;
        for item in &selected {
            cat_by_id(&client, &instances, &all_commands, &item.id, color_always).await?;
        }
        return Ok(());
    }

    let (_, cmd_id, _, _, _) = resolve_target_command(target, &all_commands, "No command")?;
    cat_by_id(&client, &instances, &all_commands, &cmd_id, color_always).await
}

/// ANSI reset escape sequence appended after color output to prevent
/// trailing color bleed into the user's terminal prompt.
pub const ANSI_RESET: &str = "\x1b[0m";

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

    let vrw = VrwClient::new(client.clone(), info);

    if color_always {
        let data = vrw.get_data(&format!("/api/commands/{}/vtty", cmd_id)).await?;
        let content = data["content"].as_str().unwrap_or("");
        print!("{}", content);
    } else {
        let data = vrw
            .get_data(&format!("/api/commands/{}/vtty/text", cmd_id))
            .await?;
        let text = data["text"].as_str().unwrap_or("");
        print!("{}", text);
    }

    Ok(())
}
