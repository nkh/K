//! CLI subcommand handlers organized by command type.

pub mod common;
pub mod config;
pub mod ipc;
pub mod list;
pub mod stop;

// vrw-only command modules
#[cfg(feature = "vrw")]
pub mod cat;
#[cfg(feature = "vrw")]
pub mod cert;
#[cfg(feature = "vrw")]
pub mod keep;
#[cfg(feature = "vrw")]
pub mod purge;
#[cfg(feature = "vrw")]
pub mod resize;
#[cfg(feature = "vrw")]
pub mod screenshot;
#[cfg(feature = "vrw")]
pub mod spawn;

// Re-export shared handlers
pub use common::c;
pub use config::handle_config_check_command;
#[cfg(not(feature = "vrw"))]
pub use ipc::{
    handle_cat_command, handle_freeze_command, handle_kill_command, handle_keys_command,
    handle_resize_command, handle_spawn_in_command, handle_thaw_command, verify_instance,
};
pub use list::{format_command, format_instance_header, handle_list_command};
#[cfg(feature = "vrw")]
pub use list::fetch_cmd_dimensions;
pub use stop::handle_stop_command;

// Re-export vrw-only handlers
#[cfg(feature = "vrw")]
pub use cat::handle_cat_command as handle_cat_command_http;
#[cfg(feature = "vrw")]
pub use cert::handle_cert_command;
#[cfg(feature = "vrw")]
pub use common::{
    collect_all_commands, format_instance_list, http_client, instance_url, post_command_action,
    resolve_instance, resolve_pid_to_id, resolve_target_command, resolve_targeted_instances,
    CommandTarget,
};
#[cfg(feature = "vrw")]
pub use list::{handle_list_commands_command, handle_list_vrw_command};
#[cfg(feature = "vrw")]
pub use keep::{handle_keep_command, handle_unkeep_command};
#[cfg(feature = "vrw")]
pub use purge::handle_purge_command;
#[cfg(feature = "vrw")]
pub use resize::handle_resize_by_pid;
#[cfg(feature = "vrw")]
pub use screenshot::handle_screenshot_command;
#[cfg(feature = "vrw")]
pub use spawn::{handle_freeze_command as handle_freeze_command_http, handle_spawn_command, handle_thaw_command as handle_thaw_command_http};
#[cfg(feature = "vrw")]
pub use stop::{handle_stop_all_commands, stop_command_by_id};
