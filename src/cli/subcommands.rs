//! CLI subcommand dispatch layer.
//!
//! Delegates to per-command handlers in the `commands` module.
//! Each handler implements the pattern: parse args → call manager/API → output result.

// Re-export all public items for backward compatibility.
// External code (main.rs, tests) imports from `crate::cli::subcommands::*`.
pub use crate::cli::commands::cert::handle_cert_command;
pub use crate::cli::commands::common::{
    c, collect_all_commands, format_instance_list, http_client, instance_url, resolve_instance,
    resolve_pid_to_id, resolve_stop_target, resolve_targeted_instances,
};
pub use crate::cli::commands::config::handle_config_check_command;
pub use crate::cli::commands::list::{
    format_command, format_instance_header, handle_list_command, handle_list_commands_command,
    handle_list_vrunner_command,
};
pub use crate::cli::commands::purge::handle_purge_command;
pub use crate::cli::commands::resize::{
    handle_resize_by_pid, handle_resize_command, resize_command_by_id,
};
pub use crate::cli::commands::spawn::{
    handle_freeze_command, handle_spawn_command, handle_thaw_command,
};
pub use crate::cli::commands::stop::{
    handle_stop_command, handle_stop_command_by_pid_on_instances, stop_command_by_id,
};
