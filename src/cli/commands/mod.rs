//! CLI subcommand handlers organized by command type.

pub mod common;
pub mod config;
pub mod ipc;
pub mod list;
pub mod stop;

// vrunner-only command modules
#[cfg(feature = "vrunner")]
pub mod cat;
#[cfg(feature = "vrunner")]
pub mod cert;
#[cfg(feature = "vrunner")]
pub mod purge;
#[cfg(feature = "vrunner")]
pub mod resize;
#[cfg(feature = "vrunner")]
pub mod screenshot;
#[cfg(feature = "vrunner")]
pub mod spawn;

// Re-export shared handlers
pub use common::c;
pub use config::handle_config_check_command;
#[cfg(not(feature = "vrunner"))]
pub use ipc::{
    handle_cat_command, handle_freeze_command, handle_kill_command, handle_keys_command,
    handle_resize_command, handle_spawn_in_command, handle_thaw_command, verify_instance,
};
pub use list::{format_command, format_instance_header, handle_list_command};
#[cfg(feature = "vrunner")]
pub use list::fetch_cmd_dimensions;
pub use stop::{handle_stop_command, resolve_stop_target};

// Re-export vrunner-only handlers
#[cfg(feature = "vrunner")]
pub use cat::handle_cat_command as handle_cat_command_http;
#[cfg(feature = "vrunner")]
pub use cert::handle_cert_command;
#[cfg(feature = "vrunner")]
pub use common::{
    collect_all_commands, format_instance_list, http_client, instance_url, post_command_action,
    resolve_instance, resolve_pid_to_id, resolve_target_command, resolve_targeted_instances,
    CommandTarget,
};
#[cfg(feature = "vrunner")]
pub use list::{handle_list_commands_command, handle_list_vrunner_command};
#[cfg(feature = "vrunner")]
pub use purge::handle_purge_command;
#[cfg(feature = "vrunner")]
pub use resize::{handle_resize_by_pid, resize_command_by_id};
#[cfg(feature = "vrunner")]
pub use screenshot::handle_screenshot_command;
#[cfg(feature = "vrunner")]
pub use spawn::{handle_freeze_command as handle_freeze_command_http, handle_spawn_command, handle_thaw_command as handle_thaw_command_http};
#[cfg(feature = "vrunner")]
pub use stop::{handle_stop_command_by_pid_on_instances, stop_command_by_id};
