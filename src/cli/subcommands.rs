//! CLI subcommand dispatch layer.
//!
//! Delegates to per-command handlers in the `commands` module.

pub use crate::cli::commands::common::{c, format_instance_list};
pub use crate::cli::commands::config::handle_config_check_command;
pub use crate::cli::commands::list::{format_command, format_instance_header, handle_list_command};
pub use crate::cli::commands::stop::{handle_stop_command, resolve_stop_target};
