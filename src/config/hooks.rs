use serde::{Deserialize, Serialize};

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
