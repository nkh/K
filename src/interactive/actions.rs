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
    /// Kill the active command.
    KillCommand,
    /// Toggle pause (freeze/thaw) on the active command.
    TogglePause,
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
        Action::KillCommand => {
            ActionEffect::KillCommand
        }
        Action::TogglePause => {
            ActionEffect::TogglePause
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
    let (term_cols, _term_rows) = terminal::size().unwrap_or((80, 24));
    let w = term_cols as usize;

    // Header
    let _ = write!(stdout, "\x1b[1;34m  vrunner — Keybindings\x1b[0m\r\n");
    let _ = write!(stdout, "\x1b[2m  {}\x1b[0m\r\n\r\n", "─".repeat(w.saturating_sub(4).min(76)));

    // Group 1: Navigation
    let nav_actions = &["NextCommand", "PrevCommand"];
    let _ = write!(stdout, "\x1b[1m  Navigation\x1b[0m\r\n");
    let mut seen = std::collections::HashSet::new();
    for binding in bindings {
        let action_name = format!("{:?}", binding.action);
        if !nav_actions.iter().any(|a| action_name.contains(a)) { continue; }
        if seen.contains(&binding.action) { continue; }
        seen.insert(binding.action.clone());
        let key_label = format_key(&binding.bytes);
        let desc = binding.action.description();
        let _ = write!(stdout, "  \x1b[1;33m{:<18}\x1b[0m  \x1b[2m{}\x1b[0m\r\n", key_label, desc);
    }
    if seen.is_empty() {
        let _ = write!(stdout, "  \x1b[2m  (none configured)\x1b[0m\r\n");
    }
    let _ = write!(stdout, "\r\n");

    // Group 2: Actions
    let action_names = &["SpawnCommand", "KillCommand", "TogglePause"];
    let _ = write!(stdout, "\x1b[1m  Actions\x1b[0m\r\n");
    for binding in bindings {
        let action_name = format!("{:?}", binding.action);
        if !action_names.iter().any(|a| action_name.contains(a)) { continue; }
        if seen.contains(&binding.action) { continue; }
        seen.insert(binding.action.clone());
        let key_label = format_key(&binding.bytes);
        let desc = binding.action.description();
        let _ = write!(stdout, "  \x1b[1;33m{:<18}\x1b[0m  \x1b[2m{}\x1b[0m\r\n", key_label, desc);
    }
    let _ = write!(stdout, "\r\n");

    // Group 3: Display
    let display_names = &["ToggleLog", "ShowHelp", "Quit"];
    let _ = write!(stdout, "\x1b[1m  Display\x1b[0m\r\n");
    for binding in bindings {
        let action_name = format!("{:?}", binding.action);
        if !display_names.iter().any(|a| action_name.contains(a)) { continue; }
        if seen.contains(&binding.action) { continue; }
        seen.insert(binding.action.clone());
        let key_label = format_key(&binding.bytes);
        let desc = binding.action.description();
        let _ = write!(stdout, "  \x1b[1;33m{:<18}\x1b[0m  \x1b[2m{}\x1b[0m\r\n", key_label, desc);
    }
    let _ = write!(stdout, "\r\n");

    // Always-active shortcuts
    let _ = write!(stdout, "\x1b[2m{}\x1b[0m\r\n", "─".repeat(w.saturating_sub(4).min(76)));
    let _ = write!(stdout, "\x1b[1m  Always active\x1b[0m\r\n");
    let _ = write!(stdout, "  \x1b[1;33m{:<18}\x1b[0m  \x1b[2mQuit display\x1b[0m\r\n", "Ctrl+\\");
    let _ = write!(stdout, "\r\n");

    // Footer
    let _ = write!(stdout, "\x1b[2m  Press any key to close\x1b[0m");
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
