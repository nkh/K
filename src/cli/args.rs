use crate::config::schema::Config;
use clap::CommandFactory;
use clap::{FromArgMatches, Parser, Subcommand};

// Binary name used in help text and completion generation.
#[cfg(feature = "vrunner")]
pub const BINARY_NAME: &str = "vrunner";
#[cfg(not(feature = "vrunner"))]
pub const BINARY_NAME: &str = "vrl";

// Description shown in --help.
#[cfg(feature = "vrunner")]
const ABOUT: &str = "A virtual terminal runner with web control plane";
#[cfg(not(feature = "vrunner"))]
const ABOUT: &str = "A virtual terminal runner";

#[derive(Parser, Debug)]
#[command(name = BINARY_NAME)]
#[command(about = ABOUT)]
#[command(trailing_var_arg = true)]
#[command(version)]
pub struct Cli {
    /// Path to configuration file
    #[arg(short, long, value_name = "FILE")]
    pub config: Option<String>,

    // ── vrunner-only args ──

    /// Server bind address (default: 127.0.0.1) [vrunner only]
    #[cfg(feature = "vrunner")]
    #[arg(short, long, value_name = "ADDR")]
    pub bind: Option<String>,

    /// Server port (default: 9090) [vrunner only]
    #[cfg(feature = "vrunner")]
    #[arg(short, long, value_name = "PORT")]
    pub port: Option<u16>,

    /// Allow remote connections (binds to 0.0.0.0 and enables auth) [vrunner only]
    #[cfg(feature = "vrunner")]
    #[arg(short, long)]
    pub remote: bool,

    /// Require authentication for API requests [vrunner only]
    #[cfg(feature = "vrunner")]
    #[arg(short, long)]
    pub auth: bool,

    /// Register this instance with another vrunner server [vrunner only]
    #[cfg(feature = "vrunner")]
    #[arg(long, value_name = "PORT")]
    pub register_with: Option<u16>,

    /// Path to the bearer token file [vrunner only]
    #[cfg(feature = "vrunner")]
    #[arg(long, value_name = "FILE")]
    pub token_file: Option<String>,

    /// Enable TLS (HTTPS) with self-signed certificates [vrunner only]
    #[cfg(feature = "vrunner")]
    #[arg(long)]
    pub tls: bool,

    /// Path to the TLS certificate file [vrunner only]
    #[cfg(feature = "vrunner")]
    #[arg(long, value_name = "FILE")]
    pub cert_file: Option<String>,

    /// Path to the TLS private key file [vrunner only]
    #[cfg(feature = "vrunner")]
    #[arg(long, value_name = "FILE")]
    pub key_file: Option<String>,

    /// Define a named certificate (repeatable) [vrunner only]
    #[cfg(feature = "vrunner")]
    #[arg(short = 'C', long, value_name = "NAME:CERT:KEY")]
    pub certificate: Option<Vec<String>>,

    // ── Shared args ──

    /// Run as a background daemon (Unix only)
    #[arg(short, long)]
    pub daemon: bool,

    /// Redirect daemon stdout to this file
    #[arg(long, value_name = "FILE")]
    pub stdout_file: Option<String>,

    /// Redirect daemon stderr to this file
    #[arg(long, value_name = "FILE")]
    pub stderr_file: Option<String>,

    /// Show VTTY on local terminal screen
    #[arg(short = 'D', long)]
    pub display: bool,

    /// Keep displaying after the initial command exits
    #[arg(short = 's', long)]
    pub display_all: bool,

    /// Disable local terminal display
    #[arg(long)]
    pub no_display: bool,

    /// Display refresh interval in milliseconds
    #[arg(long, value_name = "MS")]
    pub refresh_ms: Option<u64>,

    /// Show tab bar for command switching
    #[arg(long)]
    pub tabs: bool,

    /// Log commands to terminal
    #[arg(short, long)]
    pub log: bool,

    /// Log commands to file
    #[arg(short = 'L', long, value_name = "FILE")]
    pub log_file: Option<String>,

    /// Log raw PTY output (for debugging terminal escape sequences)
    #[arg(long, value_name = "FILE")]
    pub log_pty_raw: Option<String>,

    /// TERM value reported to child processes
    #[arg(short = 'T', long, value_name = "TERM")]
    pub term: Option<String>,

    /// VTTY rows
    #[arg(long, value_name = "N")]
    pub vtty_rows: Option<u16>,

    /// VTTY columns
    #[arg(long, value_name = "N")]
    pub vtty_cols: Option<u16>,

    /// Scrollback buffer size (number of lines)
    #[arg(long, value_name = "N")]
    pub scrollback: Option<usize>,

    /// Enable 24-bit truecolor in the virtual terminal
    #[arg(long)]
    pub truecolor: bool,

    /// Disable 24-bit truecolor in the virtual terminal
    #[arg(long)]
    pub no_truecolor: bool,

    /// Enable mouse event forwarding to child processes
    #[arg(short, long)]
    pub mouse: bool,

    /// Disable mouse event forwarding to child processes
    #[arg(long)]
    pub no_mouse: bool,

    /// Run a command when the child exits cleanly (exit code 0)
    #[arg(long, value_name = "CMD")]
    pub on_exit: Option<String>,

    /// Run a command when the child exits with an error (non-zero)
    #[arg(long, value_name = "CMD")]
    pub on_error: Option<String>,

    /// Seconds to wait before force-killing (default: 10)
    #[arg(long, value_name = "SECS")]
    pub exit_timeout: Option<u64>,

    /// Keep the VTTY buffer after the child exits
    #[arg(short = 'k', long)]
    pub retain_on_exit: bool,

    /// Save VTTY buffer to a file when the command exits
    #[arg(long, value_name = "FILE")]
    pub snapshot_on_exit: Option<String>,

    /// Send keystrokes to the command after it starts.
    #[arg(short = 'K', long, value_name = "KEYS")]
    pub send_keys: Option<String>,

    /// Set environment variables (repeatable). Format: KEY=VALUE
    #[arg(short = 'e', long, value_name = "KEY=VALUE")]
    pub env: Option<Vec<String>>,

    /// Ignore environment variables from the config file
    #[arg(short = 'E', long)]
    pub no_env: bool,

    /// Apply a named configuration profile from the config file
    #[arg(short = 'P', long, value_name = "NAME")]
    pub profile: Option<String>,

    /// Target a specific instance by PID
    #[arg(short = 't', long, value_name = "PID")]
    pub target: Option<u32>,

    /// Set the working directory for spawned commands.
    #[arg(short = 'w', long, value_name = "DIR")]
    pub working_directory: Option<String>,

    /// Subcommand
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Command to run (after -- separator)
    #[arg(trailing_var_arg = true)]
    pub cmd_args: Option<Vec<String>>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// List all running instances
    List {
        /// Interactively select instances from a numbered list
        #[arg(long)]
        interactive: bool,
    },

    /// Stop an instance by PID (auto-selects if only one)
    Stop {
        /// PID of the instance to stop
        pid: Option<u32>,
        /// Interactively select instances from a numbered list
        #[arg(long)]
        interactive: bool,
    },

    // ── vrl-only (UDS) commands ──

    /// Send keystrokes to a command in a running instance
    #[cfg(not(feature = "vrunner"))]
    Keys {
        /// PID of the target vrl instance
        pid: u32,
        /// ID of the target command (omit for first command)
        #[arg(short = 'c', long)]
        command: Option<String>,
        /// Keystrokes to send
        keys: String,
    },

    /// Show VTTY text output of a command in a running instance
    #[cfg(not(feature = "vrunner"))]
    Cat {
        /// PID of the target vrl instance
        pid: u32,
        /// ID of the target command (omit for first command)
        #[arg(short = 'c', long)]
        command: Option<String>,
        /// Interactively select commands from a numbered list
        #[arg(long)]
        interactive: bool,
    },

    /// Spawn a command inside a running instance
    #[cfg(not(feature = "vrunner"))]
    SpawnIn {
        /// PID of the target vrl instance
        pid: u32,
        /// Command to run
        cmd: String,
        /// Arguments for the command
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },

    /// Freeze (suspend) a command in a running instance
    #[cfg(not(feature = "vrunner"))]
    Freeze {
        /// PID of the target vrl instance
        pid: u32,
        /// ID of the target command (omit for first command)
        #[arg(short = 'c', long)]
        command: Option<String>,
        /// Interactively select running commands from a numbered list
        #[arg(long)]
        interactive: bool,
    },

    /// Thaw (resume) a frozen command in a running instance
    #[cfg(not(feature = "vrunner"))]
    Thaw {
        /// PID of the target vrl instance
        pid: u32,
        /// ID of the target command (omit for first command)
        #[arg(short = 'c', long)]
        command: Option<String>,
        /// Interactively select frozen commands from a numbered list
        #[arg(long)]
        interactive: bool,
    },

    /// Resize the VTTY of a command in a running instance
    #[cfg(not(feature = "vrunner"))]
    Resize {
        /// PID of the target vrl instance
        pid: u32,
        /// ID of the target command (omit for first command)
        #[arg(short = 'c', long)]
        command: Option<String>,
        /// Number of rows
        #[arg(long, default_value_t = 24)]
        rows: u16,
        /// Number of columns
        #[arg(long, default_value_t = 80)]
        cols: u16,
        /// Interactively select commands from a numbered list
        #[arg(long)]
        interactive: bool,
    },

    /// Kill (stop) a command inside a running instance
    #[cfg(not(feature = "vrunner"))]
    Kill {
        /// PID of the target vrl instance
        pid: u32,
        /// ID of the target command (omit for first command)
        #[arg(short = 'c', long)]
        command: Option<String>,
        /// Interactively select commands from a numbered list
        #[arg(long)]
        interactive: bool,
    },

    // ── vrunner-only commands ──

    /// Spawn a new command on a running vrunner instance
    #[cfg(feature = "vrunner")]
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

    /// Freeze (suspend) a running command via SIGSTOP
    #[cfg(feature = "vrunner")]
    Freeze {
        /// PID of the command to freeze
        pid: Option<u32>,
        /// Interactively select running commands from a numbered list
        #[arg(long)]
        interactive: bool,
    },

    /// Thaw (resume) a frozen command via SIGCONT
    #[cfg(feature = "vrunner")]
    Thaw {
        /// PID of the command to thaw
        pid: Option<u32>,
        /// Interactively select frozen commands from a numbered list
        #[arg(long)]
        interactive: bool,
    },

    /// Manage named certificates for per-command access control
    #[cfg(feature = "vrunner")]
    Cert {
        #[command(subcommand)]
        action: CertAction,
    },

    /// List vrunner instances (machine-readable, tab-separated)
    #[cfg(feature = "vrunner")]
    ListVrunner,

    /// List running commands (machine-readable, tab-separated)
    #[cfg(feature = "vrunner")]
    ListCommands,

    /// Stop a specific command by PID or name (not the whole instance)
    #[cfg(feature = "vrunner")]
    StopCommand {
        /// PID or name of the command to stop
        target: Option<String>,
        /// Interactively select commands from a numbered list
        #[arg(long)]
        interactive: bool,
    },

    /// Purge an exited command, discarding its VTTY buffer
    #[cfg(feature = "vrunner")]
    Purge {
        /// Command ID or name of the exited command to purge
        target: Option<String>,
    },

    /// Resize the VTTY of a running command (buffer + PTY)
    #[cfg(feature = "vrunner")]
    Resize {
        /// PID or name of the command to resize
        target: Option<String>,
        /// Number of rows (default: terminal height)
        #[arg(long, default_value_t = 0)]
        rows: u16,
        /// Number of columns (default: terminal width)
        #[arg(long, default_value_t = 0)]
        cols: u16,
        /// Interactively select commands from a numbered list
        #[arg(long)]
        interactive: bool,
    },

    /// Print the VTTY buffer of a running command as text
    #[cfg(feature = "vrunner")]
    Cat {
        /// PID or name of the command whose buffer to print
        target: Option<String>,
        /// Preserve ANSI color escape sequences in the output
        #[arg(long)]
        color_always: bool,
        /// Interactively select commands from a numbered list
        #[arg(long)]
        interactive: bool,
    },

    /// Capture the VTTY buffer as a PNG screenshot
    #[cfg(feature = "vrunner")]
    Screenshot {
        /// PID or name of the command to screenshot
        target: Option<String>,
        /// Output file path (default: <command_name>_<timestamp>.png)
        #[arg(long)]
        output: Option<String>,
        /// Font size in pixels per character cell (default: 14)
        #[arg(long, default_value_t = 14.0)]
        font_size: f32,
        /// Path to a TTF/OTF font file
        #[arg(long)]
        font_name: Option<String>,
        /// Interactively select commands from a numbered list
        #[arg(long)]
        interactive: bool,
    },

    // ── Shared commands ──

    /// Validate config files without starting
    ConfigCheck,

    /// Generate shell completion scripts
    Completions {
        /// The shell to generate completions for
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
}

/// Certificate subcommands (vrunner only).
#[cfg(feature = "vrunner")]
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
        cmd = cmd.name(BINARY_NAME);
        match cmd.clone().try_get_matches() {
            Ok(matches) => {
                <Self as FromArgMatches>::from_arg_matches(&matches)
                    .expect("failed to parse CLI arguments")
            }
            Err(err) => {
                let rendered = err.render().to_string();
                eprint!("{}", rendered);
                std::process::exit(err.exit_code());
            }
        }
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
    pub fn apply_overrides(&self, cfg: &mut Config) -> anyhow::Result<()> {
        // Server (vrunner only)
        #[cfg(feature = "vrunner")]
        {
            if let Some(bind) = &self.bind {
                cfg.server.bind = bind.clone();
            }
            if let Some(port) = self.port {
                cfg.server.port = port;
            }
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
            if self.tls {
                cfg.tls.enabled = true;
            }
            if let Some(cert_file) = &self.cert_file {
                cfg.tls.cert_file = Some(cert_file.clone());
            }
            if let Some(key_file) = &self.key_file {
                cfg.tls.key_file = Some(key_file.clone());
            }
            if let Some(cert_defs) = &self.certificate {
                for cert_def in cert_defs {
                    let parts: Vec<&str> = cert_def.splitn(3, ':').collect();
                    if parts.len() == 3 {
                        cfg.certificates.entries.push(crate::config::schema::CertificateEntryConfig {
                            name: parts[0].to_string(),
                            cert_file: parts[1].to_string(),
                            key_file: parts[2].to_string(),
                        });
                    }
                }
            }
        }

        // Daemon
        if self.daemon {
            if self.display || self.display_all || self.tabs {
                let mut flags = Vec::new();
                if self.display {
                    flags.push("--display");
                }
                if self.display_all {
                    flags.push("--display-all");
                }
                if self.tabs {
                    flags.push("--tabs");
                }
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

        // Interactive display
        if self.tabs {
            cfg.interactive.tabs = true;
        }

        // --no-env: clear config-level environment variables
        if self.no_env {
            cfg.environment.variables.clear();
        }

        // --env KEY=VALUE: merge CLI env vars into config env vars
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
        let cli = Cli::try_parse_from([BINARY_NAME, "--daemon", "--display-all", "htop"]).unwrap();
        let result = cli.apply_overrides(&mut default_config());
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("--daemon conflicts with --display-all"),
            "unexpected: {msg}"
        );
    }

    #[test]
    fn daemon_conflicts_with_display() {
        let cli = Cli::try_parse_from([BINARY_NAME, "--daemon", "--display", "htop"]).unwrap();
        let result = cli.apply_overrides(&mut default_config());
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("--daemon conflicts with --display"),
            "unexpected: {msg}"
        );
    }

    #[test]
    fn daemon_conflicts_with_tabs() {
        let cli = Cli::try_parse_from([BINARY_NAME, "--daemon", "--tabs", "htop"]).unwrap();
        let result = cli.apply_overrides(&mut default_config());
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("--daemon conflicts with --tabs"),
            "unexpected: {msg}"
        );
    }

    #[test]
    fn daemon_alone_succeeds() {
        let cli = Cli::try_parse_from([BINARY_NAME, "--daemon", "htop"]).unwrap();
        let result = cli.apply_overrides(&mut default_config());
        assert!(result.is_ok());
    }

    #[test]
    fn display_all_alone_succeeds() {
        let cli = Cli::try_parse_from([BINARY_NAME, "--display-all", "htop"]).unwrap();
        let result = cli.apply_overrides(&mut default_config());
        assert!(result.is_ok());
    }
}
