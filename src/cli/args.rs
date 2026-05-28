use crate::config::schema::Config;
use clap::{FromArgMatches, Parser, Subcommand};
use clap::CommandFactory;

#[derive(Parser, Debug)]
#[command(name = "vrunner")]
#[command(about = "A virtual terminal runner with web control plane")]
#[command(trailing_var_arg = true)]
#[command(version)]
pub struct Cli {
    /// Path to configuration file
    #[arg(short, long, value_name = "FILE")]
    pub config: Option<String>,

    /// Server bind address (default: 127.0.0.1)
    #[arg(long, value_name = "ADDR")]
    pub bind: Option<String>,

    /// Server port (default: 9090)
    #[arg(long, value_name = "PORT")]
    pub port: Option<u16>,

    /// Allow remote connections (binds to 0.0.0.0 and enables auth)
    #[arg(long)]
    pub remote: bool,

    /// Require authentication for API requests
    #[arg(long)]
    pub auth: bool,

    /// Path to the bearer token file (default: ~/.config/vrunner/token)
    #[arg(long, value_name = "FILE")]
    pub token_file: Option<String>,

    /// Enable TLS (HTTPS) with self-signed certificates
    #[arg(long)]
    pub tls: bool,

    /// Path to the TLS certificate file
    #[arg(long, value_name = "FILE")]
    pub cert_file: Option<String>,

    /// Path to the TLS private key file
    #[arg(long, value_name = "FILE")]
    pub key_file: Option<String>,

    /// Define a named certificate for the certificate pool (repeatable).
    ///
    /// Format: NAME:CERT_FILE:KEY_FILE
    /// Example: --certificate "myapp:/path/to/cert.pem:/path/to/key.pem"
    #[arg(long, value_name = "NAME:CERT:KEY")]
    pub certificate: Option<Vec<String>>,

    /// Run as a background daemon (Unix only)
    #[arg(long)]
    pub daemon: bool,

    /// Redirect daemon stdout to this file
    #[arg(long, value_name = "FILE")]
    pub stdout_file: Option<String>,

    /// Redirect daemon stderr to this file
    #[arg(long, value_name = "FILE")]
    pub stderr_file: Option<String>,

    /// Show VTTY on local terminal screen
    #[arg(long)]
    pub display: bool,

    /// Keep displaying VTTY output after the initial CLI command exits,
    /// switching to the next available command.
    ///
    /// Without this flag, the display is dismissed when the CLI command
    /// finishes but the server continues running.
    #[arg(long)]
    pub display_all: bool,

    /// Disable local terminal display
    #[arg(long)]
    pub no_display: bool,

    /// VTTY display refresh interval in milliseconds
    #[arg(long, value_name = "MS")]
    pub refresh_ms: Option<u64>,

    /// Log API commands to terminal
    #[arg(long)]
    pub log: bool,

    /// Log API commands to file
    #[arg(long, value_name = "FILE")]
    pub log_file: Option<String>,

    /// Log raw PTY output from child processes to a file.
    ///
    /// Each line records one read() call with an elapsed-time stamp and
    /// escaped bytes (printable ASCII as-is, non-printable as \xHH).
    /// The resulting log can be replayed with ansi-replay.
    #[arg(long, value_name = "FILE")]
    pub log_pty_raw: Option<String>,

    /// TERM value reported to child processes
    #[arg(long, value_name = "TERM")]
    pub term: Option<String>,

    /// VTTY rows
    #[arg(long, value_name = "N")]
    pub vtty_rows: Option<u16>,

    /// VTTY columns
    #[arg(long, value_name = "N")]
    pub vtty_cols: Option<u16>,

    /// VTTY scrollback buffer size (number of lines)
    #[arg(long, value_name = "N")]
    pub scrollback: Option<usize>,

    /// Enable 24-bit truecolor in the virtual terminal
    #[arg(long)]
    pub truecolor: bool,

    /// Disable 24-bit truecolor in the virtual terminal
    #[arg(long)]
    pub no_truecolor: bool,

    /// Enable mouse event forwarding to child processes
    #[arg(long)]
    pub mouse: bool,

    /// Disable mouse event forwarding to child processes
    #[arg(long)]
    pub no_mouse: bool,

    /// Run a command when the child exits cleanly (exit code 0)
    #[arg(long, value_name = "CMD")]
    pub on_exit: Option<String>,

    /// Run a command when the child exits with an error (non-zero exit code)
    #[arg(long, value_name = "CMD")]
    pub on_error: Option<String>,

    /// Seconds to wait for graceful exit before force-killing (default: 10)
    #[arg(long, value_name = "SECS")]
    pub exit_timeout: Option<u64>,

    /// Keep the VTTY buffer in memory after the child process exits.
    ///
    /// This is a per-command option: it only affects the command specified
    /// on the CLI, not future commands spawned via the API.
    /// The command remains visible in the tab bar and web UI with an
    /// "exited" status, allowing inspection of final output.
    /// The buffer can be manually purged via the API (DELETE /api/commands/:id).
    #[arg(long)]
    pub retain_on_exit: bool,

    /// Save the VTTY buffer to a file as plain text when the command exits.
    ///
    /// This is a per-command option. The output includes scrollback
    /// content followed by the visible screen rows, with each line
    /// trimmed of trailing whitespace. The snapshot is taken after the
    /// process exits but before the command is removed from the manager.
    #[arg(long, value_name = "FILE")]
    pub snapshot_on_exit: Option<String>,

    /// Send keystrokes to the command after it starts.
    ///
    /// Uses the same key notation as the API's send_keys endpoint.
    /// Plain text is sent as-is; special keys use <...> notation:
    ///   <Enter>, <Tab>, <Esc>, <Up>, <Down>, <Left>, <Right>,
    ///   <F1>-<F12>, <C-c>, <C-d>, <A-x> (Alt+x).
    ///
    /// Example: --send-keys "ls<Enter>"
    /// Example: --send-keys "<C-c>quit<Enter>"
    #[arg(long, value_name = "KEYS")]
    pub send_keys: Option<String>,

    /// Show tab bar for command switching in interactive display.
    ///
    /// Requires --display-all for command navigation keybindings to work.
    /// Keybindings (Ctrl+Left/Right, Ctrl+L, F12) can be customized in
    /// the config file under `interactive.keybindings`.
    #[arg(long)]
    pub tabs: bool,

    /// Set environment variables for the spawned command (repeatable).
    ///
    /// Format: KEY=VALUE
    /// Example: --env RUST_LOG=debug --env DATABASE_URL=postgres://localhost/mydb
    #[arg(long, value_name = "KEY=VALUE")]
    pub env: Option<Vec<String>>,

    /// Ignore environment variables from the config file.
    ///
    /// Only environment variables set via --env flags (CLI) or the API
    /// "env" field will be passed to the spawned command.
    #[arg(long)]
    pub no_env: bool,

    /// Apply a named configuration profile from the config file.
    /// Profile fields override the base config; CLI flags override both.
    /// Example: --profile production
    #[arg(long, value_name = "NAME")]
    pub profile: Option<String>,

    /// Target a specific vrunner instance by PID when using the spawn subcommand.
    /// If omitted and multiple instances are running, you will be prompted to choose.
    #[arg(long, value_name = "PID")]
    pub target: Option<u32>,

    /// Subcommand (list, stop, spawn, freeze, thaw, cert)
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Command to run (after -- separator)
    #[arg(trailing_var_arg = true)]
    pub cmd_args: Option<Vec<String>>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// List all running vrunner instances
    List,
    /// Stop a vrunner instance by PID.
    /// If omitted and exactly one instance is running, it is stopped automatically.
    Stop {
        /// PID of the instance to stop (optional if only one is running)
        pid: Option<u32>,
    },
    /// Spawn a new command on a running vrunner instance.
    /// If one instance is running, it is used automatically.
    /// If multiple instances exist, you will be prompted to choose.
    /// Use --target PID to skip the prompt.
    Spawn {
        /// Command to run
        cmd: String,
        /// Arguments for the command
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
        /// VTTY rows for the spawned command
        #[arg(long)]
        rows: Option<u16>,
        /// VTTY columns for the spawned command
        #[arg(long)]
        cols: Option<u16>,
    },
    /// Freeze (suspend) a running command via SIGSTOP.
    /// The command is paused but not terminated.
    Freeze {
        /// PID of the command to freeze
        pid: u32,
    },
    /// Thaw (resume) a frozen command via SIGCONT.
    Thaw {
        /// PID of the command to thaw
        pid: u32,
    },
    /// Manage named certificates for per-command access control
    Cert {
        #[command(subcommand)]
        action: CertAction,
    },
    /// List vrunner instances only (tab-separated, machine-readable)
    ListVrunner,
    /// List running commands only (tab-separated, machine-readable)
    ListCommands,
    /// Stop a specific command by PID or name (not the whole instance).
    /// If no target is given and exactly one command is running, it is stopped automatically.
    /// If the target is numeric, it is treated as a PID.
    /// Otherwise it is matched against command names (and optionally args).
    /// To match name+args, quote the full string: "vim file.txt"
    StopCommand {
        /// PID or name (and optional args, quoted) of the command to stop.
        /// If omitted and exactly one command is running, that command is stopped.
        target: Option<String>,
    },
    /// Purge a retained (exited) command from the manager.
    /// Permanently discards the VTTY buffer and all associated state.
    /// If no target is given and exactly one exited command exists, it is purged automatically.
    /// If the target is numeric, it is treated as a command ID (first 8 chars).
    /// Otherwise it is matched against command names.
    Purge {
        /// Command ID or name of the retained (exited) command to purge.
        /// If omitted and exactly one exited command exists, it is purged automatically.
        target: Option<String>,
    },
    /// Resize the VTTY of a running command.
    /// Resizes both the in-memory buffer and the child PTY (sends SIGWINCH).
    Resize {
        /// PID or name of the command to resize
        target: String,
        /// Number of rows (default: terminal height)
        #[arg(long, default_value_t = 0)]
        rows: u16,
        /// Number of columns (default: terminal width)
        #[arg(long, default_value_t = 0)]
        cols: u16,
    },
    /// Validate config files without starting the server.
    /// Reports validation errors and warnings with field paths.
    ConfigCheck,
    /// Print the VTTY buffer of a running command as plain text.
    Cat {
        /// PID or name of the command whose buffer to print.
        /// If omitted and exactly one command is running, it is used automatically.
        target: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum CertAction {
    /// Generate a new named certificate
    Generate {
        /// Name for the certificate (e.g., "webapp-frontend")
        name: String,
    },
    /// List all certificates in the pool
    List,
    /// Show details of a specific certificate
    Show {
        /// Name of the certificate to display
        name: String,
    },
    /// Remove a certificate from the pool
    Remove {
        /// Name of the certificate to remove
        name: String,
    },
}

impl Cli {
    /// Parse CLI arguments with the git SHA embedded in the version string.
    pub fn parse_with_version() -> Self {
        let version = include_str!(concat!(env!("OUT_DIR"), "/version.txt"));
        let mut cmd = <Self as CommandFactory>::command();
        cmd = cmd.version(version.trim());
        let matches = cmd.get_matches();
        <Self as FromArgMatches>::from_arg_matches(&matches)
            .expect("failed to parse CLI arguments")
    }

    /// Parse --env KEY=VALUE flags into a HashMap.
    pub fn parse_env_vars(&self) -> std::collections::HashMap<String, String> {
        let mut map = std::collections::HashMap::new();
        if let Some(env_list) = &self.env {
            for item in env_list {
                if let Some((key, value)) = item.split_once('=') {
                    map.insert(key.to_string(), value.to_string());
                }
            }
        }
        map
    }

    /// Apply CLI overrides to the loaded configuration.
    /// CLI flags take the highest precedence (override global and local config).
    pub fn apply_overrides(&self, cfg: &mut Config) -> anyhow::Result<()> {
        // Server
        if let Some(bind) = &self.bind {
            cfg.server.bind = bind.clone();
        }
        if let Some(port) = self.port {
            cfg.server.port = port;
        }

        // Security
        if self.remote {
            cfg.server.bind = "0.0.0.0".to_string();
            cfg.security.require_auth = true;
        }
        if self.auth {
            cfg.security.require_auth = true;
        }
        if let Some(token_file) = &self.token_file {
            cfg.security.token_file = token_file.clone();
        }

        // TLS
        if self.tls {
            cfg.tls.enabled = true;
        }
        if let Some(cert_file) = &self.cert_file {
            cfg.tls.cert_file = Some(cert_file.clone());
        }
        if let Some(key_file) = &self.key_file {
            cfg.tls.key_file = Some(key_file.clone());
        }

        // Certificates pool (from --certificate NAME:CERT:KEY flags)
        if let Some(cert_defs) = &self.certificate {
            for cert_def in cert_defs {
                let parts: Vec<&str> = cert_def.splitn(3, ':').collect();
                if parts.len() == 3 {
                    use crate::config::schema::CertificateEntryConfig;
                    cfg.certificates.entries.push(CertificateEntryConfig {
                        name: parts[0].to_string(),
                        cert_file: parts[1].to_string(),
                        key_file: parts[2].to_string(),
                    });
                }
            }
        }

        // Daemon
        if self.daemon {
            // --daemon detaches from the controlling terminal, making display
            // mode impossible. Reject conflicting flags early with a clear
            // message rather than silently ignoring them.
            if self.display || self.display_all || self.tabs {
                let mut flags = Vec::new();
                if self.display { flags.push("--display"); }
                if self.display_all { flags.push("--display-all"); }
                if self.tabs { flags.push("--tabs"); }
                anyhow::bail!(
                    "--daemon conflicts with {}. Display mode requires a terminal, \
                     but --daemon detaches from the controlling terminal.",
                    flags.join(", ")
                );
            }
            cfg.daemon.enabled = true;
            cfg.display.enabled = false;
        }
        if let Some(stdout_file) = &self.stdout_file {
            cfg.daemon.stdout_file = stdout_file.clone();
        }
        if let Some(stderr_file) = &self.stderr_file {
            cfg.daemon.stderr_file = stderr_file.clone();
        }

        // Display
        // --display-all implies --display: keep displaying after the CLI
        // command exits, which requires the display to be active in the
        // first place.
        if self.display || self.display_all {
            cfg.display.enabled = true;
        }
        if self.display_all {
            cfg.display.display_all = true;
        }
        if self.no_display {
            cfg.display.enabled = false;
        }
        if let Some(refresh_ms) = self.refresh_ms {
            cfg.display.refresh_ms = refresh_ms;
        }

        // Command logging
        if self.log {
            cfg.command_log.enabled = true;
        }
        if let Some(file) = &self.log_file {
            cfg.command_log.enabled = true;
            cfg.command_log.file = Some(file.clone());
        }
        if let Some(file) = &self.log_pty_raw {
            cfg.command_log.pty_raw_log = Some(file.clone());
        }

        // VTTY
        if let Some(term) = &self.term {
            cfg.vtty.term = term.clone();
        }
        if let Some(rows) = self.vtty_rows {
            cfg.vtty.rows = rows;
        }
        if let Some(cols) = self.vtty_cols {
            cfg.vtty.cols = cols;
        }
        if let Some(scrollback) = self.scrollback {
            cfg.vtty.scrollback = scrollback;
        }
        if self.truecolor {
            cfg.vtty.truecolor = true;
        }
        if self.no_truecolor {
            cfg.vtty.truecolor = false;
        }
        if self.mouse {
            cfg.vtty.mouse = true;
        }
        if self.no_mouse {
            cfg.vtty.mouse = false;
        }

        // Exit configuration
        if let Some(on_exit) = &self.on_exit {
            cfg.default_exit.exit.on_exit = Some(on_exit.clone());
        }
        if let Some(on_error) = &self.on_error {
            cfg.default_exit.exit.on_error = Some(on_error.clone());
        }
        if let Some(timeout) = self.exit_timeout {
            cfg.default_exit.exit.timeout_secs = timeout;
        }
        // Note: --retain-on-exit is NOT applied to the global default here.
        // It is a per-command option applied only to the CLI-spawned command
        // in main.rs.  Similarly, --snapshot-on-exit is per-command.

        // Interactive display
        if self.tabs {
            cfg.interactive.tabs = true;
        }

        // --no-env: clear config-level environment variables
        if self.no_env {
            cfg.environment.variables.clear();
        }

        // --env KEY=VALUE: merge CLI env vars into config env vars
        // (CLI always adds/overrides config)
        let cli_env = self.parse_env_vars();
        cfg.environment.variables.extend(cli_env);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> Config {
        Config::default()
    }

    #[test]
    fn daemon_conflicts_with_display_all() {
        let cli = Cli::try_parse_from(["vrunner", "--daemon", "--display-all", "htop"]).unwrap();
        let result = cli.apply_overrides(&mut default_config());
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("--daemon conflicts with --display-all"), "unexpected: {msg}");
    }

    #[test]
    fn daemon_conflicts_with_display() {
        let cli = Cli::try_parse_from(["vrunner", "--daemon", "--display", "htop"]).unwrap();
        let result = cli.apply_overrides(&mut default_config());
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("--daemon conflicts with --display"), "unexpected: {msg}");
    }

    #[test]
    fn daemon_conflicts_with_tabs() {
        let cli = Cli::try_parse_from(["vrunner", "--daemon", "--tabs", "htop"]).unwrap();
        let result = cli.apply_overrides(&mut default_config());
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("--daemon conflicts with --tabs"), "unexpected: {msg}");
    }

    #[test]
    fn daemon_conflicts_with_all_display_flags() {
        let cli = Cli::try_parse_from(["vrunner", "--daemon", "--display-all", "--tabs", "htop"]).unwrap();
        let result = cli.apply_overrides(&mut default_config());
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("--display-all"), "missing --display-all in: {msg}");
        assert!(msg.contains("--tabs"), "missing --tabs in: {msg}");
    }

    #[test]
    fn daemon_alone_succeeds() {
        let cli = Cli::try_parse_from(["vrunner", "--daemon", "htop"]).unwrap();
        let result = cli.apply_overrides(&mut default_config());
        assert!(result.is_ok());
    }

    #[test]
    fn display_all_alone_succeeds() {
        let cli = Cli::try_parse_from(["vrunner", "--display-all", "htop"]).unwrap();
        let result = cli.apply_overrides(&mut default_config());
        assert!(result.is_ok());
    }
}
