use serde::{Deserialize, Serialize};

/// Local terminal display settings.
/// When enabled, vrl renders VTTY output directly in the
/// terminal it was launched from (similar to mprocs).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct DisplayConfig {
    /// Show VTTY output on the local terminal.
    /// When the CLI command exits, the display is removed unless
    /// display_all is also enabled.
    pub enabled: bool,
    /// Refresh interval in milliseconds when display is enabled.
    pub refresh_ms: u64,
    /// When enabled, the display stays active after the initial CLI
    /// command exits — it switches to the next available command.
    /// When disabled (default), the display is dismissed and a status
    /// message is printed, but the server keeps running.
    pub display_all: bool,
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            refresh_ms: 100,
            display_all: false,
        }
    }
}

/// Configuration for interactive terminal display.
/// Controls keyboard input, scrolling, and command switching in the CLI.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct InteractiveConfig {
    /// Show a tab bar listing all commands at the top of the display.
    /// When disabled, the active command name is shown in the status bar only.
    #[serde(default)]
    pub tabs: bool,
    /// Configurable keybindings for the terminal display.
    /// Maps action names to human-readable key names.
    /// When a key sequence matches, the corresponding action is executed
    /// instead of forwarding the keystroke to the active command.
    ///
    /// Key name format: human-readable names.
    ///   Ctrl+Left  = "ctrl+left"
    ///   Ctrl+Right = "ctrl+right"
    ///   Ctrl+L     = "ctrl+l"
    ///   F12        = "f12"
    ///   Ctrl+H     = "ctrl+h"
    ///
    /// Raw escape sequences (e.g., "\x1b[1;5C") are also accepted
    /// for backward compatibility.
    ///
    /// Available actions:
    ///   "next_command"     — switch to the next running command (wraps around)
    ///   "prev_command"     — switch to the previous running command (wraps around)
    ///   "toggle_log"       — show/hide command log overlay
    ///   "spawn_command"    — open a prompt to spawn a new command
    ///   "show_help"        — show keybinding help overlay
    ///   "kill_command"     — kill (SIGTERM) the active command
    ///   "toggle_pause"     — pause/resume (SIGSTOP/SIGCONT) the active command
    ///   "quit"             — exit the display (same as Ctrl+\)
    #[serde(default)]
    pub keybindings: KeybindingsConfig,
}

/// Maps action names to key sequences for the interactive terminal display.
/// ```yaml
/// interactive:
///   keybindings:
///     next_command: "ctrl+right"
///     prev_command: "ctrl+left"
///     toggle_log: "ctrl+l"
///     spawn_command: "f12"
///     show_help: "ctrl+h"
///     kill_command: "ctrl+k"
///     toggle_pause: "ctrl+z"
///     quit: "esc"
/// ```
///
/// Raw escape sequences (e.g., `"\x1b[1;5C"`) are still accepted for
/// backward compatibility.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KeybindingsConfig {
    /// Switch to the next running command. Default: Ctrl+Right (`ctrl+right`)
    #[serde(default = "default_key_next_command")]
    pub next_command: Option<String>,
    /// Switch to the previous running command. Default: Ctrl+Left (`ctrl+left`)
    #[serde(default = "default_key_prev_command")]
    pub prev_command: Option<String>,
    /// Toggle the command log overlay. Default: Ctrl+L (`ctrl+l`)
    #[serde(default = "default_key_toggle_log")]
    pub toggle_log: Option<String>,
    /// Open a prompt to spawn a new command. Default: F12 (`f12`)
    #[serde(default = "default_key_spawn_command")]
    pub spawn_command: Option<String>,
    /// Show the help overlay. Default: Ctrl+H (`ctrl+h`)
    #[serde(default = "default_key_show_help")]
    pub show_help: Option<String>,
    /// Kill the active command. Default: none
    #[serde(default)]
    pub kill_command: Option<String>,
    /// Pause / resume (freeze/thaw) the active command. Default: none
    #[serde(default)]
    pub toggle_pause: Option<String>,
    /// Quit the display loop. Default: none (use Ctrl+\ = `ctrl+\\`)
    #[serde(default)]
    pub quit: Option<String>,
}

fn default_key_next_command() -> Option<String> {
    Some("ctrl+right".into())
}
fn default_key_prev_command() -> Option<String> {
    Some("ctrl+left".into())
}
fn default_key_toggle_log() -> Option<String> {
    Some("ctrl+l".into())
}
fn default_key_spawn_command() -> Option<String> {
    Some("f12".into())
}
fn default_key_show_help() -> Option<String> {
    Some("ctrl+h".into())
}

impl Default for KeybindingsConfig {
    fn default() -> Self {
        Self {
            next_command: default_key_next_command(),
            prev_command: default_key_prev_command(),
            toggle_log: default_key_toggle_log(),
            spawn_command: default_key_spawn_command(),
            show_help: default_key_show_help(),
            kill_command: None,
            toggle_pause: None,
            quit: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_config_defaults() {
        let config = DisplayConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.refresh_ms, 100);
        assert!(!config.display_all);
    }

    #[test]
    fn test_display_config_deserialize_partial() {
        let json = r#"{"enabled": true}"#;
        let config: DisplayConfig = serde_json::from_str(json).unwrap();
        assert!(config.enabled);
        // refresh_ms and display_all fall back to Rust Default impl
        assert_eq!(config.refresh_ms, 100);
        assert!(!config.display_all);
    }

    #[test]
    fn test_interactive_config_defaults() {
        let config = InteractiveConfig::default();
        assert!(!config.tabs);
        // Keybindings should have default bindings populated
        assert_eq!(
            config.keybindings.next_command.as_deref(),
            Some("ctrl+right")
        );
        assert_eq!(
            config.keybindings.prev_command.as_deref(),
            Some("ctrl+left")
        );
        assert_eq!(config.keybindings.toggle_log.as_deref(), Some("ctrl+l"));
        assert_eq!(config.keybindings.spawn_command.as_deref(), Some("f12"));
        assert_eq!(config.keybindings.show_help.as_deref(), Some("ctrl+h"));
        assert!(config.keybindings.kill_command.is_none());
        assert!(config.keybindings.toggle_pause.is_none());
        assert!(config.keybindings.quit.is_none());
    }

    #[test]
    fn test_keybindings_config_deserialize_partial() {
        let json = r#"{"next_command": "ctrl+left", "prev_command": "ctrl+right"}"#;
        let config: KeybindingsConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.next_command.as_deref(), Some("ctrl+left"));
        assert_eq!(config.prev_command.as_deref(), Some("ctrl+right"));
        // Other fields use serde defaults
        assert_eq!(config.toggle_log.as_deref(), Some("ctrl+l"));
        assert!(config.kill_command.is_none());
    }
}
