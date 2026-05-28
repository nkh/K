//! CLI subcommand handlers organized by command type.
//!
//! Each sub-module contains handlers for a specific command group:
//! - spawn/stop: process lifecycle management
//! - list: query running commands
//! - purge: remove exited commands
//! - resize: terminal dimension changes
//! - cert: TLS certificate management
//! - config: configuration validation

pub mod cert;
pub mod common;
pub mod config;
pub mod list;
pub mod purge;
pub mod resize;
pub mod spawn;
pub mod stop;

// Re-export all public handlers so that `subcommands.rs` can re-export them
// at the original paths for backward compatibility.
pub use cert::handle_cert_command;
pub use common::{
    c, collect_all_commands, format_instance_list, http_client, instance_url, resolve_instance,
    resolve_pid_to_id, resolve_stop_target, resolve_targeted_instances,
};
pub use config::handle_config_check_command;
pub use list::{
    format_command, format_instance_header, handle_list_command, handle_list_commands_command,
    handle_list_vrunner_command,
};
pub use purge::handle_purge_command;
pub use resize::{handle_resize_by_pid, handle_resize_command, resize_command_by_id};
pub use spawn::{handle_freeze_command, handle_spawn_command, handle_thaw_command};
pub use stop::{handle_stop_command, handle_stop_command_by_pid_on_instances, stop_command_by_id};
