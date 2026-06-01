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
#[derive(Debug, Clone, Deserialize, Serialize)]
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
    pub timeout_secs: u64,
    /// When true, the command's VTTY buffer is retained in memory after
    /// the child process exits.  The command appears in the display tab
    /// bar and web UI with an "exited" status, allowing inspection of
    /// the final output.  The buffer can be manually purged via the API.
    /// Default: false (commands are removed from the manager on exit).
    #[serde(default)]
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

impl Default for ExitConfig {
    fn default() -> Self {
        Self {
            on_exit: None,
            on_error: None,
            timeout_secs: default_exit_timeout(),
            retain_on_exit: false,
            snapshot_on_exit: None,
        }
    }
}

/// Default exit configuration (used when none is specified per-command).
/// The inner ExitConfig values serve as global defaults for all spawned commands.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct DefaultExitConfig {
    /// Exit behavior applied to every command unless overridden.
    #[serde(default)]
    pub exit: ExitConfig,
}

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
}

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
