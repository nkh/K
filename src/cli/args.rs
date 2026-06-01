use crate::config::schema::Config;
use clap::CommandFactory;
use clap::{FromArgMatches, Parser, Subcommand};

// Binary name used in help text and completion generation.
#[cfg(feature = "vrw")]
pub const BINARY_NAME: &str = "vrw";
#[cfg(not(feature = "vrw"))]
pub const BINARY_NAME: &str = "vrc";

// Description shown in --help.
#[cfg(feature = "vrw")]
const ABOUT: &str = "A virtual terminal runner with web control plane";
#[cfg(not(feature = "vrw"))]
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

    // ── vrw-only args ──

    /// Server bind address (default: 127.0.0.1) [vrw only]
    #[cfg(feature = "vrw")]
    #[arg(short, long, value_name = "ADDR")]
    pub bind: Option<String>,

    /// Server port (default: 9090) [vrw only]
    #[cfg(feature = "vrw")]
    #[arg(short, long, value_name = "PORT")]
    pub port: Option<u16>,

    /// Allow remote connections (binds to 0.0.0.0 and enables auth) [vrw only]
    #[cfg(feature = "vrw")]
    #[arg(short, long)]
    pub remote: bool,

    /// Require authentication for API requests [vrw only]
    #[cfg(feature = "vrw")]
    #[arg(short, long)]
    pub auth: bool,

    /// Register this instance with another vrw server [vrw only]
    #[cfg(feature = "vrw")]
    #[arg(long, value_name = "PORT")]
    pub register_with: Option<u16>,

    /// Path to the bearer token file [vrw only]
    #[cfg(feature = "vrw")]
    #[arg(long, value_name = "FILE")]
    pub token_file: Option<String>,

    /// Enable TLS (HTTPS) with self-signed certificates [vrw only]
    #[cfg(feature = "vrw")]
    #[arg(long)]
    pub tls: bool,

    /// Path to the TLS certificate file [vrw only]
    #[cfg(feature = "vrw")]
    #[arg(long, value_name = "FILE")]
    pub cert_file: Option<String>,

    /// Path to the TLS private key file [vrw only]
    #[cfg(feature = "vrw")]
    #[arg(long, value_name = "FILE")]
    pub key_file: Option<String>,

    /// Define a named certificate (repeatable) [vrw only]
    #[cfg(feature = "vrw")]
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

    /// Show VTTY on local terminal screen (keep displaying after command exits)
    #[arg(short = 'D', long)]
    pub display: bool,

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

    /// Resize the VTTY when the terminal where vrc/vrw is running is resized.
    /// By default VTTY dimensions are fixed at spawn time and only change
    /// via the programmatic resize API. With this flag, SIGWINCH events
    /// resize all VTTY buffers and PTYs to match the terminal size.
    #[arg(long)]
    pub handle_sigwinch: bool,

    /// Suppress activity logging (spawning, stopping, resizing)
    #[arg(long)]
    pub no_log: bool,

    /// Suppress terminal event output when not in --display mode.
    /// Events (spawn, exit, resize, etc.) are still logged to the log file
    /// if --log is active, but nothing is printed to the terminal.
    #[arg(long)]
    pub no_terminal_log: bool,

    /// Suppress terminal event output (short alias for --no-terminal-log)
    #[arg(short = 'q', long, hide = true)]
    pub quiet: bool,

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
        #[arg(short = 'i', long)]
        interactive: bool,
    },

    /// Stop an instance by PID (auto-selects if only one)
    Stop {
        /// PID of the instance to stop
        pid: Option<u32>,
        /// Interactively select instances from a numbered list
        #[arg(short = 'i', long)]
        interactive: bool,
    },

    // ── vrc-only (UDS) commands ──

    /// Send keystrokes to a command in a running instance
    #[cfg(not(feature = "vrw"))]
    Keys {
        /// PID of the target vrc instance
        pid: u32,
        /// ID of the target command (omit for first command)
        #[arg(short = 'c', long)]
        command: Option<String>,
        /// Keystrokes to send
        keys: String,
    },

    /// Show VTTY text output of a command in a running instance
    #[cfg(not(feature = "vrw"))]
    Cat {
        /// PID of the target vrc instance
        pid: u32,
        /// ID of the target command (omit for first command)
        #[arg(short = 'c', long)]
        command: Option<String>,
        /// Interactively select commands from a numbered list
        #[arg(short = 'i', long)]
        interactive: bool,
    },

    /// Spawn a command inside a running instance
    #[cfg(not(feature = "vrw"))]
    SpawnIn {
        /// PID of the target vrc instance
        pid: u32,
        /// Command to run
        cmd: String,
        /// Arguments for the command
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },

    /// Freeze (suspend) a command in a running instance
    #[cfg(not(feature = "vrw"))]
    Freeze {
        /// PID of the target vrc instance
        pid: u32,
        /// ID of the target command (omit for first command)
        #[arg(short = 'c', long)]
        command: Option<String>,
        /// Interactively select running commands from a numbered list
        #[arg(short = 'i', long)]
        interactive: bool,
    },

    /// Thaw (resume) a frozen command in a running instance
    #[cfg(not(feature = "vrw"))]
    Thaw {
        /// PID of the target vrc instance
        pid: u32,
        /// ID of the target command (omit for first command)
        #[arg(short = 'c', long)]
        command: Option<String>,
        /// Interactively select frozen commands from a numbered list
        #[arg(short = 'i', long)]
        interactive: bool,
    },

    /// Resize the VTTY of a command in a running instance
    #[cfg(not(feature = "vrw"))]
    Resize {
        /// PID of the target vrc instance
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
        #[arg(short = 'i', long)]
        interactive: bool,
    },

    /// Kill (stop) a command inside a running instance
    #[cfg(not(feature = "vrw"))]
    Kill {
        /// PID of the target vrc instance
        pid: u32,
        /// ID of the target command (omit for first command)
        #[arg(short = 'c', long)]
        command: Option<String>,
        /// Interactively select commands from a numbered list
        #[arg(short = 'i', long)]
        interactive: bool,
        /// Stop all commands
        #[arg(long, short = 'a')]
        all: bool,
    },

    /// Stop a command (alias for kill)
    #[cfg(not(feature = "vrw"))]
    StopCommand {
        /// PID of the target vrc instance
        pid: u32,
        /// ID of the target command (omit for first command)
        #[arg(short = 'c', long)]
        command: Option<String>,
        /// Interactively select commands from a numbered list
        #[arg(short = 'i', long)]
        interactive: bool,
        /// Stop all commands
        #[arg(long, short = 'a')]
        all: bool,
    },

    // ── vrw-only commands ──

    /// Spawn a new command on a running vrw instance
    #[cfg(feature = "vrw")]
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
    #[cfg(feature = "vrw")]
    Freeze {
        /// PID of the command to freeze
        pid: Option<u32>,
        /// Interactively select running commands from a numbered list
        #[arg(short = 'i', long)]
        interactive: bool,
    },

    /// Thaw (resume) a frozen command via SIGCONT
    #[cfg(feature = "vrw")]
    Thaw {
        /// PID of the command to thaw
        pid: Option<u32>,
        /// Interactively select frozen commands from a numbered list
        #[arg(short = 'i', long)]
        interactive: bool,
    },

    /// Manage named certificates for per-command access control
    #[cfg(feature = "vrw")]
    Cert {
        #[command(subcommand)]
        action: CertAction,
    },

    /// List vrw instances (machine-readable, tab-separated)
    #[cfg(feature = "vrw")]
    ListVrw,

    /// List running commands (machine-readable, tab-separated)
    #[cfg(feature = "vrw")]
    ListCommands,

    /// Stop a specific command by PID or name (not the whole instance)
    #[cfg(feature = "vrw")]
    StopCommand {
        /// PID or name of the command to stop
        target: Option<String>,
        /// Interactively select commands from a numbered list
        #[arg(short = 'i', long)]
        interactive: bool,
        /// Stop all commands
        #[arg(long, short = 'a')]
        all: bool,
    },

    /// Kill a command (alias for stop-command)
    #[cfg(feature = "vrw")]
    Kill {
        /// PID or name of the command to stop
        target: Option<String>,
        /// Interactively select commands from a numbered list
        #[arg(short = 'i', long)]
        interactive: bool,
        /// Stop all commands
        #[arg(long, short = 'a')]
        all: bool,
    },

    /// Purge an exited command, discarding its VTTY buffer
    #[cfg(feature = "vrw")]
    Purge {
        /// Command ID or name of the exited command to purge
        target: Option<String>,
    },

    /// Resize the VTTY of a running command (buffer + PTY)
    #[cfg(feature = "vrw")]
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
        #[arg(short = 'i', long)]
        interactive: bool,
    },

    /// Print the VTTY buffer of a running command as text
    #[cfg(feature = "vrw")]
    Cat {
        /// PID or name of the command whose buffer to print
        target: Option<String>,
        /// Strip ANSI color escape sequences; output plain text only
        #[arg(long)]
        plain: bool,
        /// Preserve ANSI color escape sequences in the output (default)
        #[arg(long, hide = true)]
        color_always: bool,
        /// Interactively select commands from a numbered list
        #[arg(short = 'i', long)]
        interactive: bool,
    },

    /// Capture the VTTY buffer as a PNG screenshot
    #[cfg(feature = "vrw")]
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
        #[arg(short = 'i', long)]
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

/// Certificate subcommands (vrw only).
#[cfg(feature = "vrw")]
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
        // Server (vrw only)
        #[cfg(feature = "vrw")]
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
            if self.display || self.tabs {
                let mut flags = Vec::new();
                if self.display {
                    flags.push("--display");
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
        if self.display {
            cfg.display.enabled = true;
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
        if self.no_log {
            cfg.command_log.enabled = false;
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

// ── vrc completion tree builder ──
//
// When both `vrc` and `vrw` features are enabled, the Commands enum only
// contains vrw variants (vrc variants are cfg'd out with `not(feature = "vrw")`).
// This means `vrc completions bash` would generate completions listing vrw-only
// subcommands (spawn, cert, purge, etc.) and flags (bind, port, etc.).
//
// This function builds a clap::Command tree that represents the vrc CLI by:
// 1. Starting from the vrw command tree (all we have at compile time)
// 2. Hiding vrw-only top-level flags and subcommands
// 3. Adding vrc-only subcommands (keys, spawn-in)
//
// The remaining shared subcommands (list, stop, freeze, thaw, resize, cat,
// kill, stop-command, config-check, completions) keep their vrw signatures,
// which is close enough for shell completion purposes.

#[cfg(all(feature = "vrc", feature = "vrw"))]
pub fn build_vrc_completions_command() -> clap::Command {
    // Hide vrw-only top-level flags and subcommands, then add vrc-only ones.
    <Cli as CommandFactory>::command()
        .name("vrc")
        .mut_arg("bind", |a| a.hide(true))
        .mut_arg("port", |a| a.hide(true))
        .mut_arg("remote", |a| a.hide(true))
        .mut_arg("auth", |a| a.hide(true))
        .mut_arg("register_with", |a| a.hide(true))
        .mut_arg("token_file", |a| a.hide(true))
        .mut_arg("tls", |a| a.hide(true))
        .mut_arg("cert_file", |a| a.hide(true))
        .mut_arg("key_file", |a| a.hide(true))
        .mut_arg("certificate", |a| a.hide(true))
        .mut_subcommand("spawn", |s| s.hide(true))
        .mut_subcommand("cert", |s| s.hide(true))
        .mut_subcommand("list-vrw", |s| s.hide(true))
        .mut_subcommand("list-commands", |s| s.hide(true))
        .mut_subcommand("purge", |s| s.hide(true))
        .mut_subcommand("screenshot", |s| s.hide(true))
        .subcommand(
            clap::Command::new("keys")
                .about("Send keystrokes to a command in a running instance")
                .arg(
                    clap::Arg::new("pid")
                        .help("PID of the target vrc instance")
                        .required(true)
                        .index(1),
                )
                .arg(
                    clap::Arg::new("command")
                        .short('c')
                        .long("command")
                        .help("ID of the target command (omit for first command)"),
                )
                .arg(
                    clap::Arg::new("keys")
                        .help("Keystrokes to send")
                        .required(true)
                        .index(2),
                ),
        )
        .subcommand(
            clap::Command::new("spawn-in")
                .about("Spawn a command inside a running instance")
                .arg(
                    clap::Arg::new("pid")
                        .help("PID of the target vrc instance")
                        .required(true)
                        .index(1),
                )
                .arg(
                    clap::Arg::new("cmd")
                        .help("Command to run")
                        .required(true)
                        .index(2),
                )
                .arg(
                    clap::Arg::new("args")
                        .help("Arguments for the command")
                        .num_args(1..)
                        .trailing_var_arg(true)
                        .index(3),
                ),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> Config {
        Config::default()
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
    fn display_implies_display_all() {
        let cli = Cli::try_parse_from([BINARY_NAME, "--display", "htop"]).unwrap();
        let mut cfg = default_config();
        let result = cli.apply_overrides(&mut cfg);
        assert!(result.is_ok());
        assert!(cfg.display.enabled);
        assert!(cfg.display.display_all);
    }

    #[test]
    fn no_log_disables_logging() {
        let cli = Cli::try_parse_from([BINARY_NAME, "--log", "--no-log", "htop"]).unwrap();
        let mut cfg = default_config();
        cfg.command_log.enabled = true;
        let result = cli.apply_overrides(&mut cfg);
        assert!(result.is_ok());
        assert!(!cfg.command_log.enabled);
    }

    #[test]
    fn interactive_short_flag_parses() {
        // Verify -i is accepted as a short form for --interactive on subcommands
        // We test that the `list` subcommand accepts -i
        let cli = Cli::try_parse_from([BINARY_NAME, "list", "-i"]).unwrap();
        match cli.command {
            Some(Commands::List { interactive }) => {
                assert!(interactive);
            }
            _ => panic!("expected List command"),
        }
    }

    #[test]
    fn handle_sigwinch_flag_parses() {
        let cli = Cli::try_parse_from([BINARY_NAME, "--handle-sigwinch", "htop"]).unwrap();
        assert!(cli.handle_sigwinch);
    }

    #[test]
    fn no_log_standalone_disables_logging() {
        let cli = Cli::try_parse_from([BINARY_NAME, "--no-log", "htop"]).unwrap();
        assert!(cli.no_log);
        assert!(!cli.log);
        let mut cfg = default_config();
        cfg.command_log.enabled = true;
        cli.apply_overrides(&mut cfg).unwrap();
        assert!(!cfg.command_log.enabled, "--no-log should disable logging");
    }

    #[test]
    fn implicit_cmd_args_captured() {
        let cli = Cli::try_parse_from([BINARY_NAME, "btop"]).unwrap();
        assert!(cli.command.is_none(), "no subcommand should be set");
        let args = cli.cmd_args.as_ref().unwrap();
        assert_eq!(args.len(), 1);
        assert_eq!(args[0], "btop");
    }

    #[test]
    fn implicit_spawn_multiple_args() {
        let cli = Cli::try_parse_from([BINARY_NAME, "cargo", "run", "--release"]).unwrap();
        assert!(cli.command.is_none());
        let args = cli.cmd_args.as_ref().unwrap();
        assert_eq!(args.len(), 3);
        assert_eq!(args[0], "cargo");
        assert_eq!(args[1], "run");
        assert_eq!(args[2], "--release");
    }

    #[test]
    fn implicit_spawn_no_flags_argc_matches() {
        // "vrw btop" → argc=2, cmd_args len=1 → 2 == 1+1 → implicit spawn
        let cli = Cli::try_parse_from([BINARY_NAME, "btop"]).unwrap();
        let cmd_args = cli.cmd_args.as_ref().unwrap();
        assert_eq!(cmd_args.len(), 1);
        // Verify argc would equal 1 + cmd_args.len()
        assert_eq!(2, 1 + cmd_args.len());
    }

    #[test]
    fn display_flag_prevents_implicit_spawn() {
        // "vrw --display htop" → argc=3, cmd_args len=1 → 3 != 1+1 → startup
        let cli = Cli::try_parse_from([BINARY_NAME, "--display", "htop"]).unwrap();
        assert!(cli.display);
        let cmd_args = cli.cmd_args.as_ref().unwrap();
        assert_eq!(cmd_args.len(), 1);
        // Verify argc would NOT equal 1 + cmd_args.len()
        assert_ne!(3, 1 + cmd_args.len(), "flags present means argc > 1 + cmd_args.len()");
    }

    #[test]
    fn daemon_flag_prevents_implicit_spawn() {
        // "vrw --daemon htop" → argc=3, cmd_args len=1 → startup
        let cli = Cli::try_parse_from([BINARY_NAME, "--daemon", "htop"]).unwrap();
        assert!(cli.daemon);
        let cmd_args = cli.cmd_args.as_ref().unwrap();
        assert_ne!(3, 1 + cmd_args.len());
    }

    #[test]
    fn multiple_flags_prevent_implicit_spawn() {
        // "vrw --display --tabs --log htop" → argc=5, cmd_args len=1 → startup
        let cli = Cli::try_parse_from([BINARY_NAME, "--display", "--tabs", "--log", "htop"]).unwrap();
        let cmd_args = cli.cmd_args.as_ref().unwrap();
        assert_eq!(cmd_args.len(), 1);
        assert_ne!(5, 1 + cmd_args.len());
    }

    // ── kill / stop-command alias tests ──

    #[cfg(feature = "vrw")]
    #[test]
    fn kill_command_parses() {
        let cli = Cli::try_parse_from([BINARY_NAME, "kill", "42"]).unwrap();
        match cli.command {
            Some(Commands::Kill { target, interactive, all }) => {
                assert_eq!(target.as_deref(), Some("42"));
                assert!(!interactive);
                assert!(!all);
            }
            _ => panic!("expected Kill command, got {:?}", cli.command),
        }
    }

    #[cfg(feature = "vrw")]
    #[test]
    fn stop_command_parses() {
        let cli = Cli::try_parse_from([BINARY_NAME, "stop-command", "btop"]).unwrap();
        match cli.command {
            Some(Commands::StopCommand { target, interactive, all }) => {
                assert_eq!(target.as_deref(), Some("btop"));
                assert!(!interactive);
                assert!(!all);
            }
            _ => panic!("expected StopCommand command, got {:?}", cli.command),
        }
    }

    #[cfg(feature = "vrw")]
    #[test]
    fn kill_and_stop_command_same_fields() {
        // Both aliases should parse the same arguments identically
        let cli_kill = Cli::try_parse_from([BINARY_NAME, "kill", "--all", "-i"]).unwrap();
        let cli_stop = Cli::try_parse_from([BINARY_NAME, "stop-command", "--all", "-i"]).unwrap();

        match (&cli_kill.command, &cli_stop.command) {
            (
                Some(Commands::Kill { target: t1, interactive: i1, all: a1 }),
                Some(Commands::StopCommand { target: t2, interactive: i2, all: a2 }),
            ) => {
                assert_eq!(t1, t2, "target field should match");
                assert_eq!(i1, i2, "interactive field should match");
                assert_eq!(a1, a2, "all field should match");
                assert!(*a1, "--all should be true");
                assert!(*i1, "-i should be true");
            }
            _ => panic!("expected Kill and StopCommand"),
        }
    }

    // ── kill --all / stop-command --all tests ──

    #[cfg(feature = "vrw")]
    #[test]
    fn kill_all_flag_parses() {
        let cli = Cli::try_parse_from([BINARY_NAME, "kill", "--all"]).unwrap();
        match cli.command {
            Some(Commands::Kill { target, all, .. }) => {
                assert!(target.is_none(), "--all should not require a target");
                assert!(all);
            }
            _ => panic!("expected Kill command"),
        }
    }

    #[cfg(feature = "vrw")]
    #[test]
    fn stop_command_all_flag_parses() {
        let cli = Cli::try_parse_from([BINARY_NAME, "stop-command", "--all"]).unwrap();
        match cli.command {
            Some(Commands::StopCommand { target, all, .. }) => {
                assert!(target.is_none(), "--all should not require a target");
                assert!(all);
            }
            _ => panic!("expected StopCommand command"),
        }
    }

    #[cfg(feature = "vrw")]
    #[test]
    fn kill_all_short_flag_parses() {
        let cli = Cli::try_parse_from([BINARY_NAME, "kill", "-a"]).unwrap();
        match cli.command {
            Some(Commands::Kill { all, .. }) => {
                assert!(all);
            }
            _ => panic!("expected Kill command"),
        }
    }

    #[cfg(feature = "vrw")]
    #[test]
    fn stop_command_all_short_flag_parses() {
        let cli = Cli::try_parse_from([BINARY_NAME, "stop-command", "-a"]).unwrap();
        match cli.command {
            Some(Commands::StopCommand { all, .. }) => {
                assert!(all);
            }
            _ => panic!("expected StopCommand command"),
        }
    }

    // ── --interactive (-i) tests for all vrw subcommands that have it ──

    #[cfg(feature = "vrw")]
    #[test]
    fn stop_command_interactive_flag_parses() {
        let cli = Cli::try_parse_from([BINARY_NAME, "stop-command", "-i"]).unwrap();
        match cli.command {
            Some(Commands::StopCommand { interactive, .. }) => {
                assert!(interactive);
            }
            _ => panic!("expected StopCommand command"),
        }
    }

    #[cfg(feature = "vrw")]
    #[test]
    fn kill_interactive_flag_parses() {
        let cli = Cli::try_parse_from([BINARY_NAME, "kill", "-i"]).unwrap();
        match cli.command {
            Some(Commands::Kill { interactive, .. }) => {
                assert!(interactive);
            }
            _ => panic!("expected Kill command"),
        }
    }

    #[cfg(feature = "vrw")]
    #[test]
    fn cat_interactive_flag_parses() {
        let cli = Cli::try_parse_from([BINARY_NAME, "cat", "-i"]).unwrap();
        match cli.command {
            Some(Commands::Cat { interactive, .. }) => {
                assert!(interactive);
            }
            _ => panic!("expected Cat command"),
        }
    }

    #[cfg(feature = "vrw")]
    #[test]
    fn freeze_interactive_flag_parses() {
        let cli = Cli::try_parse_from([BINARY_NAME, "freeze", "-i"]).unwrap();
        match cli.command {
            Some(Commands::Freeze { interactive, .. }) => {
                assert!(interactive);
            }
            _ => panic!("expected Freeze command"),
        }
    }

    #[cfg(feature = "vrw")]
    #[test]
    fn thaw_interactive_flag_parses() {
        let cli = Cli::try_parse_from([BINARY_NAME, "thaw", "-i"]).unwrap();
        match cli.command {
            Some(Commands::Thaw { interactive, .. }) => {
                assert!(interactive);
            }
            _ => panic!("expected Thaw command"),
        }
    }

    #[cfg(feature = "vrw")]
    #[test]
    fn resize_interactive_flag_parses() {
        let cli = Cli::try_parse_from([BINARY_NAME, "resize", "-i"]).unwrap();
        match cli.command {
            Some(Commands::Resize { interactive, .. }) => {
                assert!(interactive);
            }
            _ => panic!("expected Resize command"),
        }
    }

    #[cfg(feature = "vrw")]
    #[test]
    fn screenshot_interactive_flag_parses() {
        let cli = Cli::try_parse_from([BINARY_NAME, "screenshot", "-i"]).unwrap();
        match cli.command {
            Some(Commands::Screenshot { interactive, .. }) => {
                assert!(interactive);
            }
            _ => panic!("expected Screenshot command"),
        }
    }

    #[cfg(feature = "vrw")]
    #[test]
    fn stop_interactive_flag_parses() {
        let cli = Cli::try_parse_from([BINARY_NAME, "stop", "-i"]).unwrap();
        match cli.command {
            Some(Commands::Stop { interactive, .. }) => {
                assert!(interactive);
            }
            _ => panic!("expected Stop command"),
        }
    }

    // ── cat color_always / plain tests ──

    #[cfg(feature = "vrw")]
    #[test]
    fn cat_plain_flag_parses() {
        let cli = Cli::try_parse_from([BINARY_NAME, "cat", "--plain", "btop"]).unwrap();
        match cli.command {
            Some(Commands::Cat { plain, color_always, interactive, .. }) => {
                assert!(plain);
                assert!(!color_always);
                assert!(!interactive);
            }
            _ => panic!("expected Cat command"),
        }
    }

    #[cfg(feature = "vrw")]
    #[test]
    fn cat_no_flags_defaults_color() {
        let cli = Cli::try_parse_from([BINARY_NAME, "cat", "btop"]).unwrap();
        match cli.command {
            Some(Commands::Cat { plain, color_always, .. }) => {
                assert!(!plain, "plain should default to false");
                assert!(!color_always, "color_always should default to false (implicit)");
            }
            _ => panic!("expected Cat command"),
        }
    }

    // ── daemon + no_log coexistence ──

    #[test]
    fn daemon_with_no_log_succeeds() {
        let cli = Cli::try_parse_from([BINARY_NAME, "--daemon", "--no-log", "htop"]).unwrap();
        let mut cfg = default_config();
        let result = cli.apply_overrides(&mut cfg);
        assert!(result.is_ok(), "--daemon + --no-log should not conflict");
        assert!(cfg.daemon.enabled);
        assert!(!cfg.command_log.enabled, "--no-log should disable logging");
    }

    #[test]
    fn quiet_short_flag_parses() {
        let cli = Cli::try_parse_from([BINARY_NAME, "-q", "htop"]).unwrap();
        assert!(cli.quiet);
        assert!(!cli.no_log, "-q should not set --no-log field directly");
        assert!(!cli.no_terminal_log, "-q should not set --no-terminal-log field directly");
    }

    #[test]
    fn quiet_long_flag_parses() {
        let cli = Cli::try_parse_from([BINARY_NAME, "--quiet", "htop"]).unwrap();
        assert!(cli.quiet);
    }

    #[test]
    fn no_terminal_log_flag_parses() {
        let cli = Cli::try_parse_from([BINARY_NAME, "--no-terminal-log", "htop"]).unwrap();
        assert!(cli.no_terminal_log);
        assert!(!cli.quiet, "--no-terminal-log should not set quiet");
        assert!(!cli.no_log, "--no-terminal-log should not set no_log");
    }

    #[test]
    fn quiet_does_not_disable_logging() {
        // --quiet only suppresses terminal output, not command logging
        let cli = Cli::try_parse_from([BINARY_NAME, "--quiet", "htop"]).unwrap();
        assert!(cli.quiet);
        let mut cfg = default_config();
        cfg.command_log.enabled = true;
        cli.apply_overrides(&mut cfg).unwrap();
        assert!(cfg.command_log.enabled, "--quiet should NOT disable logging");
    }

    #[test]
    fn quiet_short_does_not_disable_logging() {
        let cli = Cli::try_parse_from([BINARY_NAME, "-q", "htop"]).unwrap();
        let mut cfg = default_config();
        cfg.command_log.enabled = true;
        cli.apply_overrides(&mut cfg).unwrap();
        assert!(cfg.command_log.enabled, "-q should NOT disable logging");
    }

    #[test]
    fn no_log_still_disables_logging() {
        let cli = Cli::try_parse_from([BINARY_NAME, "--no-log", "htop"]).unwrap();
        let mut cfg = default_config();
        cfg.command_log.enabled = true;
        cli.apply_overrides(&mut cfg).unwrap();
        assert!(!cfg.command_log.enabled, "--no-log should disable logging");
    }

    #[test]
    fn quiet_with_daemon_succeeds() {
        let cli = Cli::try_parse_from([BINARY_NAME, "--daemon", "-q", "htop"]).unwrap();
        let mut cfg = default_config();
        let result = cli.apply_overrides(&mut cfg);
        assert!(result.is_ok(), "--daemon + -q should not conflict");
        assert!(cfg.daemon.enabled);
        // -q no longer disables logging
    }

    // ── env var parsing ──

    #[test]
    fn env_vars_parsed_correctly() {
        let cli = Cli::try_parse_from([
            BINARY_NAME,
            "-e", "FOO=bar",
            "-e", "BAZ=qux",
            "htop",
        ]).unwrap();
        let env_map = cli.parse_env_vars();
        assert_eq!(env_map.len(), 2);
        assert_eq!(env_map.get("FOO").unwrap(), "bar");
        assert_eq!(env_map.get("BAZ").unwrap(), "qux");
    }

    #[test]
    fn env_vars_applied_to_config() {
        let cli = Cli::try_parse_from([BINARY_NAME, "-e", "MY_VAR=hello", "htop"]).unwrap();
        let mut cfg = default_config();
        cfg.environment.variables.insert("PREEXIST".to_string(), "val".to_string());
        cli.apply_overrides(&mut cfg).unwrap();
        assert_eq!(cfg.environment.variables.get("MY_VAR").unwrap(), "hello");
        assert_eq!(cfg.environment.variables.get("PREEXIST").unwrap(), "val");
    }

    #[test]
    fn no_env_clears_config_env() {
        let cli = Cli::try_parse_from([BINARY_NAME, "--no-env", "htop"]).unwrap();
        let mut cfg = default_config();
        cfg.environment.variables.insert("SHOULD_BE_CLEARED".to_string(), "x".to_string());
        cli.apply_overrides(&mut cfg).unwrap();
        assert!(cfg.environment.variables.is_empty(), "--no-env should clear config env vars");
    }

    // ── log_file override ──

    #[test]
    fn log_file_enables_logging() {
        let cli = Cli::try_parse_from([BINARY_NAME, "--log-file", "/tmp/k.log", "htop"]).unwrap();
        let mut cfg = default_config();
        cfg.command_log.enabled = false;
        cli.apply_overrides(&mut cfg).unwrap();
        assert!(cfg.command_log.enabled, "--log-file should enable logging");
        assert_eq!(cfg.command_log.file.as_deref(), Some("/tmp/k.log"));
    }

    // ── display + no_display conflict resolution ──

    #[test]
    fn no_display_overrides_display() {
        let cli = Cli::try_parse_from([BINARY_NAME, "--display", "--no-display", "htop"]).unwrap();
        let mut cfg = default_config();
        cli.apply_overrides(&mut cfg).unwrap();
        // --display is processed first (sets enabled=true), then --no-display (sets enabled=false)
        assert!(!cfg.display.enabled, "--no-display should override --display");
    }

    // ── truecolor / no_truecolor ──

    #[test]
    fn truecolor_overrides_config() {
        let cli = Cli::try_parse_from([BINARY_NAME, "--truecolor", "htop"]).unwrap();
        let mut cfg = default_config();
        cfg.vtty.truecolor = false;
        cli.apply_overrides(&mut cfg).unwrap();
        assert!(cfg.vtty.truecolor);
    }

    #[test]
    fn no_truecolor_overrides_config() {
        let cli = Cli::try_parse_from([BINARY_NAME, "--no-truecolor", "htop"]).unwrap();
        let mut cfg = default_config();
        cfg.vtty.truecolor = true;
        cli.apply_overrides(&mut cfg).unwrap();
        assert!(!cfg.vtty.truecolor);
    }

    // ── mouse / no_mouse ──

    #[test]
    fn mouse_enables_in_config() {
        let cli = Cli::try_parse_from([BINARY_NAME, "--mouse", "htop"]).unwrap();
        let mut cfg = default_config();
        cfg.vtty.mouse = false;
        cli.apply_overrides(&mut cfg).unwrap();
        assert!(cfg.vtty.mouse);
    }

    #[test]
    fn no_mouse_disables_in_config() {
        let cli = Cli::try_parse_from([BINARY_NAME, "--no-mouse", "htop"]).unwrap();
        let mut cfg = default_config();
        cfg.vtty.mouse = true;
        cli.apply_overrides(&mut cfg).unwrap();
        assert!(!cfg.vtty.mouse);
    }

    // ── refresh_ms ──

    #[test]
    fn refresh_ms_applied_to_config() {
        let cli = Cli::try_parse_from([BINARY_NAME, "--refresh-ms", "500", "htop"]).unwrap();
        let mut cfg = default_config();
        cli.apply_overrides(&mut cfg).unwrap();
        assert_eq!(cfg.display.refresh_ms, 500);
    }

    // ── vtty_rows / vtty_cols ──

    #[test]
    fn vtty_dims_applied_to_config() {
        let cli = Cli::try_parse_from([BINARY_NAME, "--vtty-rows", "50", "--vtty-cols", "160", "htop"]).unwrap();
        let mut cfg = default_config();
        cli.apply_overrides(&mut cfg).unwrap();
        assert_eq!(cfg.vtty.rows, 50);
        assert_eq!(cfg.vtty.cols, 160);
    }

    // ── scrollback ──

    #[test]
    fn scrollback_applied_to_config() {
        let cli = Cli::try_parse_from([BINARY_NAME, "--scrollback", "10000", "htop"]).unwrap();
        let mut cfg = default_config();
        cli.apply_overrides(&mut cfg).unwrap();
        assert_eq!(cfg.vtty.scrollback, 10000);
    }

    // ── tabs ──

    #[test]
    fn tabs_enables_in_config() {
        let cli = Cli::try_parse_from([BINARY_NAME, "--tabs", "htop"]).unwrap();
        let mut cfg = default_config();
        cli.apply_overrides(&mut cfg).unwrap();
        assert!(cfg.interactive.tabs);
    }

    // ── retain_on_exit ──

    #[test]
    fn retain_on_exit_flag_parses() {
        let cli = Cli::try_parse_from([BINARY_NAME, "--retain-on-exit", "htop"]).unwrap();
        assert!(cli.retain_on_exit);
    }

    // ── config-check subcommand ──

    #[test]
    fn config_check_subcommand_parses() {
        let cli = Cli::try_parse_from([BINARY_NAME, "config-check"]).unwrap();
        match cli.command {
            Some(Commands::ConfigCheck) => {}
            _ => panic!("expected ConfigCheck command"),
        }
    }

    // ── completions subcommand ──

    #[test]
    fn completions_subcommand_parses() {
        let cli = Cli::try_parse_from([BINARY_NAME, "completions", "bash"]).unwrap();
        match cli.command {
            Some(Commands::Completions { shell }) => {
                assert_eq!(shell, clap_complete::Shell::Bash);
            }
            _ => panic!("expected Completions command"),
        }
    }

    // ── vrc-specific kill / stop-command alias tests (cfg(not(feature = "vrw"))) ──

    #[cfg(not(feature = "vrw"))]
    #[test]
    fn vrc_kill_command_parses() {
        let cli = Cli::try_parse_from([BINARY_NAME, "kill", "1234", "-c", "cmd1"]).unwrap();
        match cli.command {
            Some(Commands::Kill { pid, command, interactive, all }) => {
                assert_eq!(pid, 1234);
                assert_eq!(command.as_deref(), Some("cmd1"));
                assert!(!interactive);
                assert!(!all);
            }
            _ => panic!("expected Kill command"),
        }
    }

    #[cfg(not(feature = "vrw"))]
    #[test]
    fn vrc_stop_command_parses() {
        let cli = Cli::try_parse_from([BINARY_NAME, "stop-command", "1234", "-c", "cmd1"]).unwrap();
        match cli.command {
            Some(Commands::StopCommand { pid, command, interactive, all }) => {
                assert_eq!(pid, 1234);
                assert_eq!(command.as_deref(), Some("cmd1"));
                assert!(!interactive);
                assert!(!all);
            }
            _ => panic!("expected StopCommand command"),
        }
    }

    #[cfg(not(feature = "vrw"))]
    #[test]
    fn vrc_kill_and_stop_command_same_fields() {
        let cli_kill = Cli::try_parse_from([BINARY_NAME, "kill", "1234", "--all", "-i"]).unwrap();
        let cli_stop = Cli::try_parse_from([BINARY_NAME, "stop-command", "1234", "--all", "-i"]).unwrap();

        match (&cli_kill.command, &cli_stop.command) {
            (
                Some(Commands::Kill { pid: p1, command: c1, interactive: i1, all: a1 }),
                Some(Commands::StopCommand { pid: p2, command: c2, interactive: i2, all: a2 }),
            ) => {
                assert_eq!(p1, p2, "pid field should match");
                assert_eq!(c1, c2, "command field should match");
                assert_eq!(i1, i2, "interactive field should match");
                assert_eq!(a1, a2, "all field should match");
                assert!(*a1, "--all should be true");
                assert!(*i1, "-i should be true");
            }
            _ => panic!("expected Kill and StopCommand"),
        }
    }

    #[cfg(not(feature = "vrw"))]
    #[test]
    fn vrc_kill_all_flag_parses() {
        let cli = Cli::try_parse_from([BINARY_NAME, "kill", "1234", "--all"]).unwrap();
        match cli.command {
            Some(Commands::Kill { all, .. }) => {
                assert!(all);
            }
            _ => panic!("expected Kill command"),
        }
    }

    #[cfg(not(feature = "vrw"))]
    #[test]
    fn vrc_stop_command_all_flag_parses() {
        let cli = Cli::try_parse_from([BINARY_NAME, "stop-command", "1234", "--all"]).unwrap();
        match cli.command {
            Some(Commands::StopCommand { all, .. }) => {
                assert!(all);
            }
            _ => panic!("expected StopCommand command"),
        }
    }
}
