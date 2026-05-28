//! Interactive terminal features: keybindings, actions, help overlay, spawn prompt,
//! and the main display loop.
//!
//! This module provides all the interactive functionality for the terminal display
//! loop, including:
//!
//! - **Display loop**: The core interactive rendering loop that displays VTTY
//!   buffers, forwards keystrokes, handles overlays, and manages terminal state.
//! - **Key name parsing**: Converts human-readable key names (e.g., `ctrl+left`,
//!   `f12`, `enter`) to raw byte sequences for terminal matching.
//! - **Keybinding resolution**: Builds a lookup table from config keybindings,
//!   supporting both readable names and raw escape notation for backward compat.
//! - **Action dispatch**: Executes bound actions (next/prev command, toggle log,
//!   spawn command, show help, quit).
//! - **Help overlay**: Renders a full-screen help overlay listing all configured
//!   keybindings and their descriptions.
//! - **Spawn prompt**: Leaves raw mode temporarily to accept a command string
//!   from the user, spawns it via the `CommandManager`, and returns to raw mode.
//!
//! # Config format
//!
//! Keybindings in the config file use human-readable names:
//!
//! ```yaml
//! interactive:
//!   keybindings:
//!     next_command: "ctrl+right"
//!     prev_command: "ctrl+left"
//!     toggle_log: "ctrl+l"
//!     spawn_command: "f12"
//!     show_help: "ctrl+h"
//!     kill_command: "ctrl+k"
//!     toggle_pause: "ctrl+z"
//!     quit: "esc"
//! ```
//!
//! Raw escape sequences (e.g., `"\x1b[1;5C"`) are still accepted for backward
//! compatibility, but readable names are strongly preferred.

mod actions;
pub mod display;
mod keybinding;

pub use actions::{
    execute_action, read_spawn_command, render_help_overlay, render_spawn_prompt, restore_raw_mode,
    ActionEffect,
};
pub use keybinding::{check_bindings, parse_key_name, resolve_keybindings, Action, Binding};
