//! CLI subcommand dispatch layer.
//!
//! Delegates to per-command handlers in the `commands` module.

pub use crate::cli::commands::common::c;
pub use crate::cli::commands::config::handle_config_check_command;

// vrc-specific re-exports
#[cfg(not(feature = "vrw"))]
pub use crate::cli::commands::ipc::{
    handle_cat_command, handle_freeze_command, handle_kill_all_commands, handle_kill_command, handle_keys_command,
    handle_resize_command, handle_spawn_in_command, handle_thaw_command,
};
#[cfg(not(feature = "vrw"))]
pub use crate::cli::commands::list::{format_command, format_instance_header, handle_list_command};
#[cfg(not(feature = "vrw"))]
pub use crate::cli::commands::stop::{handle_stop_command, resolve_stop_target};

// vrw-specific re-exports
#[cfg(feature = "vrw")]
pub use crate::cli::commands::common::http_client;
#[cfg(feature = "vrw")]
pub use crate::cli::commands::cat::handle_cat_command as handle_cat_command_http;
#[cfg(feature = "vrw")]
pub use crate::cli::commands::cert::handle_cert_command;
#[cfg(feature = "vrw")]
pub use crate::cli::commands::list::{
    format_command, format_instance_header, handle_list_command, handle_list_commands_command,
    handle_list_vrw_command,
};
#[cfg(feature = "vrw")]
pub use crate::cli::commands::keep::{handle_keep_command, handle_unkeep_command};
#[cfg(feature = "vrw")]
pub use crate::cli::commands::purge::handle_purge_command;
#[cfg(feature = "vrw")]
pub use crate::cli::commands::resize::{handle_resize_command, resize_command_by_id};
#[cfg(feature = "vrw")]
pub use crate::cli::commands::screenshot::handle_screenshot_command;
#[cfg(feature = "vrw")]
pub use crate::cli::commands::spawn::{handle_spawn_command, handle_thaw_command as handle_thaw_command_http, handle_freeze_command as handle_freeze_command_http};
#[cfg(feature = "vrw")]
pub use crate::cli::commands::stop::{handle_stop_all_commands, handle_stop_command, resolve_stop_target};

// ── Shared helper: fetch UDS command list for interactive selection ──

/// Fetch the list of commands from a vrc instance via UDS and return them
/// as `SelectItem` list for interactive selection.
#[cfg(not(feature = "vrw"))]
pub async fn fetch_uds_command_items(
    pid: u32,
) -> anyhow::Result<Vec<crate::cli::interactive_select::SelectItem>> {
    use crate::ipc::client::send_command;
    use crate::ipc::protocol::{ControlCommand, ControlResponse};

    let response = send_command(pid, ControlCommand::List).await?;
    let commands = match response {
        ControlResponse::Ok { data } => data
            .get("commands")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default(),
        ControlResponse::Error { error } => anyhow::bail!("{}", error),
    };

    Ok(commands
        .iter()
        .filter_map(|cmd| {
            let id = cmd.get("id")?.as_str()?.to_string();
            let name = cmd.get("name")?.as_str()?.to_string();
            let cmd_pid = cmd.get("pid")?.as_u64()?;
            Some(crate::cli::interactive_select::SelectItem {
                label: format!("{} (PID {})", name, cmd_pid),
                id,
            })
        })
        .collect())
}

// ── Interactive handler stubs (vrc) ──
// These are thin wrappers that fetch the command list via UDS, present an
// interactive selection, then call the existing per-command handler for
// each selected item.

#[cfg(not(feature = "vrw"))]
pub async fn handle_cat_command_interactive(pid: u32) -> anyhow::Result<()> {
    use crate::cli::interactive_select::select_items;
    let items = fetch_uds_command_items(pid).await?;
    let selected = select_items(&items, "Select commands to cat [space-separated numbers]")?;
    for item in &selected {
        handle_cat_command(pid, Some(&item.id)).await?;
    }
    Ok(())
}

#[cfg(not(feature = "vrw"))]
pub async fn handle_freeze_command_interactive(pid: u32) -> anyhow::Result<()> {
    use crate::cli::interactive_select::select_items;
    let items = fetch_uds_command_items(pid).await?;
    let selected = select_items(&items, "Select commands to freeze [space-separated numbers]")?;
    for item in &selected {
        handle_freeze_command(pid, Some(&item.id)).await?;
    }
    Ok(())
}

#[cfg(not(feature = "vrw"))]
pub async fn handle_thaw_command_interactive(pid: u32) -> anyhow::Result<()> {
    use crate::cli::interactive_select::select_items;
    let items = fetch_uds_command_items(pid).await?;
    let selected = select_items(&items, "Select commands to thaw [space-separated numbers]")?;
    for item in &selected {
        handle_thaw_command(pid, Some(&item.id)).await?;
    }
    Ok(())
}

#[cfg(not(feature = "vrw"))]
pub async fn handle_resize_command_interactive(pid: u32, rows: u16, cols: u16) -> anyhow::Result<()> {
    use crate::cli::interactive_select::select_items;
    let items = fetch_uds_command_items(pid).await?;
    let selected = select_items(&items, "Select commands to resize [space-separated numbers]")?;
    for item in &selected {
        handle_resize_command(pid, Some(&item.id), rows, cols).await?;
    }
    Ok(())
}

#[cfg(not(feature = "vrw"))]
pub async fn handle_kill_command_interactive(pid: u32) -> anyhow::Result<()> {
    use crate::cli::interactive_select::select_items;
    let items = fetch_uds_command_items(pid).await?;
    let selected = select_items(&items, "Select commands to kill [space-separated numbers]")?;
    for item in &selected {
        handle_kill_command(pid, Some(&item.id)).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    // subcommands.rs is primarily a re-export module with IPC-dependent handlers.
    // We verify the re-exports compile and have correct signatures.

    #[test]
    fn test_handle_config_check_command_callable() {
        // handle_config_check_command is always available (shared)
        let _ = super::handle_config_check_command;
    }

    #[test]
    fn test_c_callable() {
        let _ = super::c;
    }

    #[test]
    fn test_c_is_function_pointer() {
        // c is a pub function pointer — verify it can be called without panic
        let result = super::c("test", crossterm::style::Color::White, false);
        assert!(!result.is_empty());
    }

    #[test]
    fn test_config_check_signature() {
        // Verify handle_config_check_command exists and has correct type
        // (it returns Result, so we verify it's callable)
        let _fn_ptr: fn(Option<&str>) -> Result<(), anyhow::Error> = super::handle_config_check_command;
    }

    #[cfg(feature = "vrw")]
    #[test]
    fn test_vrw_re_exports_compile() {
        // Verify all vrw re-exports are accessible
        let _ = super::http_client;
        let _ = super::format_command;
        let _ = super::format_instance_header;
        let _ = super::handle_cert_command;
        let _ = super::handle_keep_command;
        let _ = super::handle_unkeep_command;
        let _ = super::handle_purge_command;
        let _ = super::handle_resize_command;
        let _ = super::handle_screenshot_command;
        let _ = super::handle_cat_command_http;
        let _ = super::handle_spawn_command;
        let _ = super::handle_list_command;
        let _ = super::handle_list_vrw_command;
        let _ = super::handle_list_commands_command;
        let _ = super::handle_stop_command;
        let _ = super::handle_stop_all_commands;
        let _ = super::resolve_stop_target;
    }

    #[cfg(not(feature = "vrw"))]
    #[test]
    fn test_vrc_re_exports_compile() {
        // Verify all vrc re-exports are accessible
        let _ = super::format_command;
        let _ = super::format_instance_header;
        let _ = super::handle_cat_command;
        let _ = super::handle_freeze_command;
        let _ = super::handle_kill_command;
        let _ = super::handle_keys_command;
        let _ = super::handle_resize_command;
        let _ = super::handle_spawn_in_command;
        let _ = super::handle_thaw_command;
        let _ = super::handle_list_command;
        let _ = super::handle_stop_command;
        let _ = super::resolve_stop_target;
    }
}



