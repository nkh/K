use serde::{Deserialize, Serialize};

/// Event hooks configuration.
/// Allows registering shell commands that run on lifecycle events.
///
/// Example:
/// ```yaml
/// hooks:
///   on_spawn: "notify-send 'Started' {name}"
///   on_exit: "echo done"
///   on_kill: "echo killed {name} (pid={pid})"
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct HooksConfig {
    /// Command to run when ANY child process is spawned.
    /// Placeholders: {name}, {id}, {pid}
    #[serde(default)]
    pub on_spawn: Option<String>,
    /// Command to run when ANY child process exits cleanly (exit code 0).
    /// Placeholders: {name}, {id}, {pid}, {exit_code}
    #[serde(default)]
    pub on_exit: Option<String>,
    /// Command to run when ANY child process exits with error (non-zero).
    /// Placeholders: {name}, {id}, {pid}, {exit_code}
    #[serde(default)]
    pub on_error: Option<String>,
    /// Command to run when ANY child process is killed.
    /// Placeholders: {name}, {id}, {pid}
    #[serde(default)]
    pub on_kill: Option<String>,
}

/// Exit configuration for a command.
/// Controls what happens when a command exits, including cleanup commands and timeouts.
/// This can be set per-command via the spawn API or as defaults in the config.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ExitConfig {
    /// Command to run when the child exits cleanly (exit code 0).
    /// The string is split on whitespace into a binary and arguments.
    /// Example: "notify-send Build OK"
    /// Set to null to disable.
    #[serde(default)]
    pub on_exit: Option<String>,
    /// Command to run when the child exits with a non-zero code.
    /// Example: "notify-send Build FAILED"
    /// Set to null to disable.
    #[serde(default)]
    pub on_error: Option<String>,
    /// Maximum seconds to wait for the child to exit after SIGTERM
    /// before sending SIGKILL. Default: 10 seconds.
    /// Applies when kill is called or when the server shuts down.
    #[serde(default = "default_exit_timeout")]
    #[default = "default_exit_timeout()"]
    pub timeout_secs: u64,
    /// When true, the command's VTTY buffer is retained in memory after
    /// the child process exits.  The command appears in the display tab
    /// bar and web UI with an "exited" status, allowing inspection of
    /// the final output.  The buffer can be manually purged via the API.
    /// Default: false (commands are removed from the manager on exit).
    #[serde(default)]
    #[default]
    pub retain_on_exit: bool,
    /// When set to a file path, the VTTY buffer is saved to that file
    /// as plain text when the child process exits.  The snapshot is taken
    /// after the process exits but before the command is removed from the
    /// manager.  This is a per-command option (set via CLI or API).
    ///
    /// The output includes scrollback content followed by the visible
    /// screen rows.  Each line is trimmed of trailing whitespace.
    ///
    /// Example: --snapshot-on-exit /tmp/htop-output.txt
    #[serde(default)]
    pub snapshot_on_exit: Option<String>,
}

fn default_exit_timeout() -> u64 {
    10
}


/// Default exit configuration (used when none is specified per-command).
/// The inner ExitConfig values serve as global defaults for all spawned commands.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct DefaultExitConfig {
    /// Exit behavior applied to every command unless overridden.
    #[serde(default)]
    pub exit: ExitConfig,
}

/// ANSI color code for a single log field.
/// Stores the raw escape sequence (e.g. "\x1b[32m" for green).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ColorField {
    /// ANSI SGR escape sequence.  Empty string means no color (reset).
    /// Common values:
    ///   ""              — default/reset
    ///   "\x1b[90m"       — dark grey
    ///   "\x1b[32m"       — green
    ///   "\x1b[1;37m"      — bright white
    ///   "\x1b[34m"       — blue
    ///   "\x1b[1;33m"      — bright yellow
    ///   "\x1b[32m"       — dark green
    ///   "\x1b[1;32m"      — bright green
    ///   "\x1b[1;31m"      — bright red
    ///   "\x1b[1;35m"      — bright magenta
    ///   "\x1b[1;36m"      — bright cyan
    #[serde(default)]
    pub ansi: String,
}

/// Terminal log appearance configuration.
/// Controls which fields are shown, their colors, padding, and the output format.
///
/// Example:
/// ```yaml
/// command_log:
///   enabled: true
///   terminal:
///     format: "%timestamp% %pid% %cmd% %event% %details%"
///     colors:
///       timestamp: { ansi: "\x1b[90m" }
///       pid: { ansi: "\x1b[1;37m" }
///       cmd: { ansi: "\x1b[32m" }
///       event: { ansi: "\x1b[32m" }
///     pad:
///       pid: 6
///       cmd: 16
///       event: 17
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct TerminalLogConfig {
    /// Printf-like format string controlling which fields appear in the
    /// terminal log line and in what order.  Available placeholders:
    ///
    ///   %timestamp%  — wall-clock time (HH:MM:SS.cc)
    ///   %pid%        — child process ID
    ///   %id%         — internal command UUID (first 8 chars)
    ///   %cmd%        — command name (binary path basename)
    ///   %event%      — rvw event type (spawn, resize, kill, …)
    ///   %details%    — remaining key=value pairs
    ///
    /// Default: "%timestamp% %pid% %cmd% %event% %details%"
    #[serde(default = "default_terminal_format")]
    #[default = "default_terminal_format()"]
    pub format: String,
    /// Per-field ANSI color configuration.  Each field maps to a ColorField
    /// containing the ANSI SGR escape sequence.
    #[serde(default)]
    pub colors: TerminalLogColors,
    /// Per-field padding widths.  Fields are padded (right-aligned) to this
    /// width and truncated if longer.
    #[serde(default)]
    pub pad: TerminalLogPad,
}

fn default_terminal_format() -> String {
    "%timestamp% %pid% %cmd% %event% %details%".to_string()
}

/// Per-field ANSI color settings for terminal log output.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct TerminalLogColors {
    #[serde(default = "default_clr_timestamp")]
    pub timestamp: ColorField,
    #[serde(default = "default_clr_pid")]
    pub pid: ColorField,
    #[serde(default = "default_clr_id")]
    pub id: ColorField,
    #[serde(default = "default_clr_cmd")]
    pub cmd: ColorField,
    #[serde(default = "default_clr_event")]
    pub event: ColorField,
    #[serde(default = "default_clr_arg")]
    pub arg: ColorField,
    #[serde(default = "default_clr_cert")]
    pub cert: ColorField,
    #[serde(default = "default_clr_env")]
    pub env: ColorField,
    #[serde(default = "default_clr_size")]
    pub size: ColorField,
    #[serde(default = "default_clr_dir")]
    pub dir: ColorField,
    #[serde(default = "default_clr_detail")]
    pub detail: ColorField,
}

fn default_clr_timestamp() -> ColorField { ColorField { ansi: "\x1b[90m".to_string() } }
fn default_clr_pid() -> ColorField { ColorField { ansi: "\x1b[1;37m".to_string() } }
fn default_clr_id() -> ColorField { ColorField { ansi: "\x1b[32m".to_string() } }
fn default_clr_cmd() -> ColorField { ColorField { ansi: "\x1b[32m".to_string() } }
fn default_clr_event() -> ColorField { ColorField { ansi: "\x1b[32m".to_string() } }
fn default_clr_arg() -> ColorField { ColorField { ansi: "\x1b[32m".to_string() } }
fn default_clr_cert() -> ColorField { ColorField { ansi: "\x1b[34m".to_string() } }
fn default_clr_env() -> ColorField { ColorField { ansi: "\x1b[32m".to_string() } }
fn default_clr_size() -> ColorField { ColorField { ansi: "\x1b[1;33m".to_string() } }
fn default_clr_dir() -> ColorField { ColorField { ansi: "\x1b[34m".to_string() } }
fn default_clr_detail() -> ColorField { ColorField { ansi: "\x1b[90m".to_string() } }


/// Per-field padding widths for terminal log output.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct TerminalLogPad {
    /// PID field width (default: 6)
    #[serde(default = "default_pad_pid")]
    #[default = "default_pad_pid()"]
    pub pid: usize,
    /// Command name field width (default: 16)
    #[serde(default = "default_pad_cmd")]
    #[default = "default_pad_cmd()"]
    pub cmd: usize,
    /// Event type field width (default: 17, fits "thaw_keybinding")
    #[serde(default = "default_pad_event")]
    #[default = "default_pad_event()"]
    pub event: usize,
}

fn default_pad_pid() -> usize { 6 }
fn default_pad_cmd() -> usize { 16 }
fn default_pad_event() -> usize { 17 }



/// Command logging configuration.
/// Records API command events (spawn, kill, resize, etc.) to a log file.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct CommandLogConfig {
    /// Enable logging of API commands.
    #[serde(default)]
    pub enabled: bool,
    /// Path to the command log file. If set, logs are written to this file
    /// in addition to the terminal.
    #[serde(default)]
    pub file: Option<String>,
    /// Path to a file where raw PTY output from child processes is logged.
    /// Each line contains one `read()` call's worth of data from the PTY
    /// master fd, formatted with elapsed time and escaped bytes:
    ///
    ///   <elapsed_ms> <escaped_bytes>
    ///
    /// Printable ASCII is written as-is; non-printable bytes use \xHH
    /// notation.  This produces a human-readable yet machine-parseable
    /// log that can be replayed step-by-step with the `ansi-replay` tool.
    ///
    /// Set via CLI: `--log-pty-raw <FILE>`
    /// Set via config:
    ///   command_log:
    ///     pty_raw_log: "/tmp/pty-output.log"
    #[serde(default)]
    pub pty_raw_log: Option<String>,
    /// Terminal log appearance and format configuration.
    /// Controls which fields are shown, their colors, padding, and layout.
    ///
    /// Set via config:
    ///   command_log:
    ///     terminal:
    ///       format: "%timestamp% %pid% %cmd% %event% %details%"
    ///       colors:
    ///         timestamp: { ansi: "\x1b[90m" }
    ///         pid: { ansi: "\x1b[1;37m" }
    ///       pad:
    ///         pid: 6
    ///         cmd: 16
    ///         event: 17
    #[serde(default)]
    pub terminal: TerminalLogConfig,
}

// Default derived: all fields have natural defaults

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hooks_config_defaults() {
        let hooks = HooksConfig::default();
        assert!(hooks.on_spawn.is_none());
        assert!(hooks.on_exit.is_none());
        assert!(hooks.on_error.is_none());
        assert!(hooks.on_kill.is_none());
    }

    #[test]
    fn test_hooks_config_partial() {
        let hooks = HooksConfig {
            on_spawn: Some("echo spawn".to_string()),
            on_exit: Some("echo exit".to_string()),
            ..Default::default()
        };
        assert_eq!(hooks.on_spawn.as_deref(), Some("echo spawn"));
        assert_eq!(hooks.on_exit.as_deref(), Some("echo exit"));
        assert!(hooks.on_error.is_none());
        assert!(hooks.on_kill.is_none());
    }

    #[test]
    fn test_hooks_config_all_fields() {
        let hooks = HooksConfig {
            on_spawn: Some("notify-send 'Started' {name}".to_string()),
            on_exit: Some("echo '{name} exited successfully'".to_string()),
            on_error: Some("notify-send 'vrc' '{name} failed (exit {exit_code})'".to_string()),
            on_kill: Some("echo 'Killed {name}'".to_string()),
        };
        assert!(hooks.on_spawn.is_some());
        assert!(hooks.on_exit.is_some());
        assert!(hooks.on_error.is_some());
        assert!(hooks.on_kill.is_some());
    }

    #[test]
    fn test_hooks_config_default_trait() {
        let hooks = HooksConfig::default();
        assert!(hooks.on_spawn.is_none());
        assert!(hooks.on_exit.is_none());
        assert!(hooks.on_error.is_none());
        assert!(hooks.on_kill.is_none());
    }
}
