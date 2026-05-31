use crate::config::schema::Config;
use clap::CommandFactory;
use clap::{FromArgMatches, Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "vrl")]
#[command(about = "A virtual terminal runner")]
#[command(trailing_var_arg = true)]
#[command(version)]
pub struct Cli {
    /// Path to configuration file
    #[arg(short, long, value_name = "FILE")]
    pub config: Option<String>,

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
    /// Special keys use <...> notation, e.g. <Enter> <C-c> <Esc>
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

    /// Target a specific vrl instance by PID
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
    /// List all running vrl instances
    List,

    /// Stop a vrl instance by PID (auto-selects if only one)
    Stop {
        /// PID of the instance to stop
        pid: Option<u32>,
    },

    /// Send keystrokes to a command in a running instance
    Keys {
        /// PID of the target vrl instance
        pid: u32,
        /// ID of the target command (omit for first command)
        #[arg(short = 'c', long)]
        command: Option<String>,
        /// Keystrokes to send. Use <Enter>, <C-c>, <Esc> etc. for special keys.
        keys: String,
    },

    /// Show VTTY text output of a command in a running instance
    Cat {
        /// PID of the target vrl instance
        pid: u32,
        /// ID of the target command (omit for first command)
        #[arg(short = 'c', long)]
        command: Option<String>,
    },

    /// Spawn a command inside a running vrl instance
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
    Freeze {
        /// PID of the target vrl instance
        pid: u32,
        /// ID of the target command (omit for first command)
        #[arg(short = 'c', long)]
        command: Option<String>,
    },

    /// Thaw (resume) a frozen command in a running instance
    Thaw {
        /// PID of the target vrl instance
        pid: u32,
        /// ID of the target command (omit for first command)
        #[arg(short = 'c', long)]
        command: Option<String>,
    },

    /// Resize the VTTY of a command in a running instance
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
    },

    /// Validate config files without starting
    ConfigCheck,

    /// Generate shell completion scripts for vrl
    Completions {
        /// The shell to generate completions for
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
}

impl Cli {
    /// Parse CLI arguments with the git SHA embedded in the version string.
    pub fn parse_with_version() -> Self {
        let version = include_str!(concat!(env!("OUT_DIR"), "/version.txt"));
        let mut cmd = <Self as CommandFactory>::command();
        cmd = cmd.version(version.trim());
        cmd = cmd.name("vrl");
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
    /// CLI flags take the highest precedence (override global and local config).
    pub fn apply_overrides(&self, cfg: &mut Config) -> anyhow::Result<()> {
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
        let cli = Cli::try_parse_from(["vrunner", "--daemon", "--display-all", "htop"]).unwrap();
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
        let cli = Cli::try_parse_from(["vrunner", "--daemon", "--display", "htop"]).unwrap();
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
        let cli = Cli::try_parse_from(["vrunner", "--daemon", "--tabs", "htop"]).unwrap();
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
