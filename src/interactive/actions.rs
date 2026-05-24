//! Action implementations for interactive terminal display.
//!
//! Provides the side effects triggered by keybindings: rendering help overlays,
//! running spawn prompts, and (via return values) signalling the display loop
//! to switch commands or quit.

use std::io::Write;

use crossterm::terminal;

use super::keybinding::{Action, Binding, format_key};

/// The result of executing an action in the display loop.
#[derive(Debug, Clone, PartialEq)]
pub enum ActionEffect {
    /// No special effect (action handled internally).
    None,
    /// Switch to the next command.
    NextCommand,
    /// Switch to the previous command.
    PrevCommand,
    /// Toggle the log overlay on/off.  `true` = show, `false` = hide.
    ToggleLog(bool),
    /// Show the help overlay.
    ShowHelp,
    /// Quit the display loop.
    Quit,
}

/// Dispatch a matched action to its handler, returning the effect to apply.
///
/// This function does NOT perform side effects like rendering directly (except
/// for `ShowHelp` which needs to render immediately).  Most effects are
/// returned as enum variants so the display loop can manage state transitions.
pub fn execute_action(
    action: &Action,
    showing_log: bool,
    display_all: bool,
    command_count: usize,
    _bindings: &[Binding],
) -> ActionEffect {
    match action {
        Action::NextCommand => {
            if display_all && command_count > 1 {
                ActionEffect::NextCommand
            } else {
                ActionEffect::None
            }
        }
        Action::PrevCommand => {
            if display_all && command_count > 1 {
                ActionEffect::PrevCommand
            } else {
                ActionEffect::None
            }
        }
        Action::ToggleLog => {
            let show = !showing_log;
            ActionEffect::ToggleLog(show)
        }
        Action::SpawnCommand => {
            ActionEffect::None // Handled separately by the display loop
        }
        Action::ShowHelp => {
            ActionEffect::ShowHelp
        }
        Action::Quit => {
            ActionEffect::Quit
        }
    }
}

/// Render a full-screen help overlay showing all configured keybindings.
///
/// The overlay replaces the VTTY display.  Callers should set a flag
/// (`showing_help = true`) so the display loop knows to re-render the
/// overlay on tick instead of the VTTY buffer.
pub fn render_help_overlay(
    bindings: &[Binding],
    stdout: &mut std::io::Stdout,
) {
    use std::io::Write;

    let _ = terminal::Clear(terminal::ClearType::All);

    let _term_size = terminal::size().unwrap_or((80, 24));

    // Header
    let _ = write!(stdout, "\x1b[1;34m  vrunner — Keybindings\x1b[0m\r\n\r\n");

    // Group bindings: list each unique action once with its key
    let mut seen = std::collections::HashSet::new();
    for binding in bindings {
        if seen.contains(&binding.action) {
            continue;
        }
        seen.insert(binding.action.clone());

        let key_label = format_key(&binding.bytes);
        let desc = binding.action.description();
        let _ = write!(stdout, "  \x1b[1;33m{:<20}\x1b[0m  {}\r\n", key_label, desc);
    }

    // Hardcoded shortcuts
    let _ = write!(stdout, "\r\n  \x1b[2mHardcoded shortcuts:\x1b[0m\r\n");
    let hardcoded = [
        ("Ctrl+\\", "Quit display (always active)"),
        ("q / Ctrl+C", "Shut down (when dismissed)"),
    ];
    for (key, desc) in &hardcoded {
        let _ = write!(stdout, "  \x1b[2m{:<20}  {}\x1b[0m\r\n", key, desc);
    }

    // Footer
    let _ = write!(stdout, "\r\n  \x1b[2mPress any key to close\x1b[0m");
    let _ = stdout.flush();
}

/// Render a spawn prompt overlay (placeholder for future use).
#[allow(dead_code)]
pub fn render_spawn_prompt(stdout: &mut std::io::Stdout) {
    let _ = terminal::Clear(terminal::ClearType::All);
    let _ = write!(stdout, "\x1b[1;34m  vrunner — Spawn Command\x1b[0m\r\n\r\n");
    let _ = write!(stdout, "  \x1b[1mCommand:\x1b[0m ");
    let _ = stdout.flush();
}

/// Read a command string from the user in cooked (non-raw) mode.
///
/// Temporarily disables raw mode, reads a line from stdin, then re-enables
/// raw mode.  Returns `Some(input)` on Enter, `None` on Ctrl+C / empty.
pub fn read_spawn_command() -> Option<String> {
    // Leave raw mode so the user gets normal line editing
    let _ = terminal::disable_raw_mode();

    let mut input = String::new();
    eprint!("\r\n  Command: ");
    let _ = std::io::stderr().flush();

    match std::io::stdin().read_line(&mut input) {
        Ok(0) => None,
        Ok(_) => {
            let trimmed = input.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        }
        Err(_) => None,
    }
}

/// Re-enable raw mode after spawn prompt.
pub fn restore_raw_mode() -> bool {
    if let Err(e) = terminal::enable_raw_mode() {
        tracing::warn!(error = %e, "Failed to re-enable raw mode after spawn prompt");
        return false;
    }
    true
}
