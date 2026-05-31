//! CLI subcommand handlers organized by command type.
//!
//! Each sub-module contains handlers for a specific command group:
//! - list: query running instances (PID file based)
//! - stop: stop running instances (signal based)
//! - config: configuration validation
//! - ipc: inter-instance communication via UDS

pub mod common;
pub mod config;
pub mod ipc;
pub mod list;
pub mod stop;

// Re-export all public handlers
pub use common::{c, format_instance_list};
pub use config::handle_config_check_command;
pub use ipc::{
    handle_cat_command, handle_freeze_command, handle_keys_command, handle_resize_command,
    handle_spawn_in_command, handle_thaw_command, verify_instance,
};
pub use list::{format_command, format_instance_header, handle_list_command};
pub use stop::{handle_stop_command, resolve_stop_target};
