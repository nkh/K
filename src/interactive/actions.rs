//! Action implementations for interactive terminal display.
//!
//! Provides the side effects triggered by keybindings: rendering help overlays,
//! running spawn prompts, and (via return values) signalling the display loop
//! to switch commands or quit.

use std::io::Write;

use crossterm::terminal;

use super::keybinding::{format_key, Action, Binding};

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
    command_count: usize,
    _bindings: &[Binding],
) -> ActionEffect {
    match action {
        Action::NextCommand => {
            if command_count > 1 {
                ActionEffect::NextCommand
            } else {
                ActionEffect::None
            }
        }
        Action::PrevCommand => {
            if command_count > 1 {
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
        Action::ShowHelp => ActionEffect::ShowHelp,
        Action::KillCommand => ActionEffect::KillCommand,
        Action::TogglePause => ActionEffect::TogglePause,
        Action::Quit => ActionEffect::Quit,
    }
}

/// Render a full-screen help overlay showing all configured keybindings.
///
/// The overlay replaces the VTTY display.  Callers should set a flag
/// (`showing_help = true`) so the display loop knows to re-render the
/// overlay on tick instead of the VTTY buffer.
pub fn render_help_overlay(bindings: &[Binding], stdout: &mut std::io::Stdout) {
    use std::io::Write;

    let _ = terminal::Clear(terminal::ClearType::All);
    // Ensure cursor is at home position after clear
    let _ = write!(stdout, "\x1b[H");
    let (term_cols, _term_rows) = terminal::size().unwrap_or((80, 24));
    let w = term_cols as usize;

    // Header
    let _ = write!(
        stdout,
        "\x1b[1;34m  vrc \u{2014} Keybindings\x1b[0m\r\n"
    );
    let _ = write!(
        stdout,
        "\x1b[2m  {}\x1b[0m\r\n\r\n",
        "\u{2500}".repeat(w.saturating_sub(4).min(76))
    );

    // Track which actions we've already displayed to avoid duplicates
    // across groups.  The "Always active" section is handled separately
    // and is not part of the bindings list.
    let mut seen = std::collections::HashSet::new();

    /// Helper: render one group of keybindings.
    fn render_group(
        stdout: &mut std::io::Stdout,
        group_name: &str,
        action_names: &[&str],
        bindings: &[Binding],
        seen: &mut std::collections::HashSet<Action>,
    ) {
        let _ = write!(stdout, "\x1b[1m  {}\x1b[0m\r\n", group_name);
        let mut had_any = false;
        for binding in bindings {
            let action_name = format!("{:?}", binding.action);
            if !action_names.iter().any(|a| action_name.contains(a)) {
                continue;
            }
            if seen.contains(&binding.action) {
                continue;
            }
            seen.insert(binding.action.clone());
            had_any = true;
            let key_label = format_key(&binding.bytes);
            let desc = binding.action.description();
            let _ = write!(
                stdout,
                "  \x1b[1;33m{:<18}\x1b[0m  \x1b[2m{}\x1b[0m\r\n",
                key_label, desc
            );
        }
        if !had_any {
            let _ = write!(stdout, "  \x1b[2m  (none configured)\x1b[0m\r\n");
        }
        let _ = write!(stdout, "\r\n");
    }

    // Group 1: Navigation
    render_group(
        stdout,
        "Navigation",
        &["NextCommand", "PrevCommand"],
        bindings,
        &mut seen,
    );

    // Group 2: Actions
    render_group(
        stdout,
        "Actions",
        &["SpawnCommand", "KillCommand", "TogglePause"],
        bindings,
        &mut seen,
    );

    // Group 3: Display
    render_group(
        stdout,
        "Display",
        &["ToggleLog", "ShowHelp", "Quit"],
        bindings,
        &mut seen,
    );

    // Always-active shortcuts (hardcoded, not from config)
    let _ = write!(
        stdout,
        "\x1b[2m{}\x1b[0m\r\n",
        "\u{2500}".repeat(w.saturating_sub(4).min(76))
    );
    let _ = write!(stdout, "\x1b[1m  Always active\x1b[0m\r\n");
    let _ = write!(
        stdout,
        "  \x1b[1;33m{:<18}\x1b[0m  \x1b[2mQuit display\x1b[0m\r\n",
        "Ctrl+\\"
    );
    let _ = write!(stdout, "\r\n");

    // Footer
    let _ = write!(stdout, "\x1b[2m  Press any key to close\x1b[0m");
    let _ = stdout.flush();
}

/// Render a spawn prompt overlay (placeholder for future use).
#[allow(dead_code)]
pub fn render_spawn_prompt(stdout: &mut std::io::Stdout) {
    let _ = terminal::Clear(terminal::ClearType::All);
    let _ = write!(stdout, "\x1b[1;34m  vrc — Spawn Command\x1b[0m\r\n\r\n");
    let _ = write!(stdout, "  \x1b[1mCommand:\x1b[0m ");
    let _ = stdout.flush();
}

/// Read a command string from the user in cooked (non-raw) mode.
///
/// Temporarily disables raw mode, moves the cursor to the bottom of the
/// terminal, reads a line from stdin, then re-enables raw mode.
/// Returns `Some(input)` on Enter, `None` on Ctrl+C / empty.
pub fn read_spawn_command() -> Option<String> {
    use std::io::Write;

    // Leave raw mode so the user gets normal line editing
    let _ = terminal::disable_raw_mode();

    // Move cursor to the last line of the terminal
    let (_, term_rows) = terminal::size().unwrap_or((80, 24));
    eprint!("\x1b[{};1H", term_rows); // cursor to (term_rows, 1)
                                      // Clear from cursor to end of line
    eprint!("\x1b[2K");
    eprint!("\x1b[1;36m  Spawn:\x1b[0m ");
    let _ = std::io::stderr().flush();

    let mut input = String::new();

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interactive::keybinding::{Action, Binding};

    fn sample_bindings() -> Vec<Binding> {
        vec![]
    }

    #[test]
    fn test_execute_next_command_single() {
        let effect = execute_action(&Action::NextCommand, false, 1, &sample_bindings());
        assert_eq!(effect, ActionEffect::None);
    }

    #[test]
    fn test_execute_next_command_multiple() {
        let effect = execute_action(&Action::NextCommand, false, 3, &sample_bindings());
        assert_eq!(effect, ActionEffect::NextCommand);
    }

    #[test]
    fn test_execute_prev_command_single() {
        let effect = execute_action(&Action::PrevCommand, false, 1, &sample_bindings());
        assert_eq!(effect, ActionEffect::None);
    }

    #[test]
    fn test_execute_prev_command_multiple() {
        let effect = execute_action(&Action::PrevCommand, false, 5, &sample_bindings());
        assert_eq!(effect, ActionEffect::PrevCommand);
    }

    #[test]
    fn test_execute_toggle_log_off() {
        let effect = execute_action(&Action::ToggleLog, false, 1, &sample_bindings());
        assert_eq!(effect, ActionEffect::ToggleLog(true));
    }

    #[test]
    fn test_execute_toggle_log_on() {
        let effect = execute_action(&Action::ToggleLog, true, 1, &sample_bindings());
        assert_eq!(effect, ActionEffect::ToggleLog(false));
    }

    #[test]
    fn test_execute_spawn_command() {
        let effect = execute_action(&Action::SpawnCommand, false, 1, &sample_bindings());
        assert_eq!(effect, ActionEffect::None);
    }

    #[test]
    fn test_execute_show_help() {
        let effect = execute_action(&Action::ShowHelp, false, 1, &sample_bindings());
        assert_eq!(effect, ActionEffect::ShowHelp);
    }

    #[test]
    fn test_execute_kill_command() {
        let effect = execute_action(&Action::KillCommand, false, 1, &sample_bindings());
        assert_eq!(effect, ActionEffect::KillCommand);
    }

    #[test]
    fn test_execute_toggle_pause() {
        let effect = execute_action(&Action::TogglePause, false, 1, &sample_bindings());
        assert_eq!(effect, ActionEffect::TogglePause);
    }

    #[test]
    fn test_execute_quit() {
        let effect = execute_action(&Action::Quit, false, 1, &sample_bindings());
        assert_eq!(effect, ActionEffect::Quit);
    }
}
