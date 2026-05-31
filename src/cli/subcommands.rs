//! CLI subcommand dispatch layer.
//!
//! Delegates to per-command handlers in the `commands` module.

pub use crate::cli::commands::common::c;
pub use crate::cli::commands::config::handle_config_check_command;

// vrl-specific re-exports
#[cfg(not(feature = "vrunner"))]
pub use crate::cli::commands::ipc::{
    handle_cat_command, handle_freeze_command, handle_kill_command, handle_keys_command,
    handle_resize_command, handle_spawn_in_command, handle_thaw_command,
};
#[cfg(not(feature = "vrunner"))]
pub use crate::cli::commands::list::{format_command, format_instance_header, handle_list_command};
#[cfg(not(feature = "vrunner"))]
pub use crate::cli::commands::stop::{handle_stop_command, resolve_stop_target};

// vrunner-specific re-exports
#[cfg(feature = "vrunner")]
pub use crate::cli::commands::cat::handle_cat_command as handle_cat_command_http;
#[cfg(feature = "vrunner")]
pub use crate::cli::commands::cert::handle_cert_command;
#[cfg(feature = "vrunner")]
pub use crate::cli::commands::list::{
    format_command, format_instance_header, handle_list_command, handle_list_commands_command,
    handle_list_vrunner_command,
};
#[cfg(feature = "vrunner")]
pub use crate::cli::commands::purge::handle_purge_command;
#[cfg(feature = "vrunner")]
pub use crate::cli::commands::resize::{handle_resize_command, resize_command_by_id};
#[cfg(feature = "vrunner")]
pub use crate::cli::commands::screenshot::handle_screenshot_command;
#[cfg(feature = "vrunner")]
pub use crate::cli::commands::spawn::{handle_spawn_command, handle_thaw_command as handle_thaw_command_http, handle_freeze_command as handle_freeze_command_http};
#[cfg(feature = "vrunner")]
pub use crate::cli::commands::stop::{handle_stop_command, resolve_stop_target};
