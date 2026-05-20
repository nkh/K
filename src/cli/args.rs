use clap::{Parser, Subcommand};
use crate::config::schema::Config;

#[derive(Parser, Debug)]
#[command(name = "vrunner")]
#[command(about = "A virtual terminal runner with web control plane")]
#[command(trailing_var_arg = true)]
pub struct Cli {
    /// Path to configuration file
    #[arg(short, long, value_name = "FILE")]
    pub config: Option<String>,

    /// Server bind address (default: 127.0.0.1)
    #[arg(long, value_name = "ADDR")]
    pub bind: Option<String>,

    /// Server port (default: 8080)
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

    /// Define a named certificate for the certificate pool (can be repeated).
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

    /// Show tab bar for command switching in interactive display
    #[arg(long)]
    pub tabs: bool,

    /// Set environment variables for the spawned command (can be repeated).
    /// Format: KEY=VALUE
    /// Example: --env RUST_LOG=debug --env DATABASE_URL=postgres://localhost/mydb
    #[arg(long, value_name = "KEY=VALUE")]
    pub env: Option<Vec<String>>,

    /// Ignore environment variables from the config file.
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
    /// Stop a vrunner instance by PID
    Stop {
        /// PID of the instance to stop
        pid: u32,
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
    },
    /// Freeze (suspend) a running command via SIGSTOP.
    /// The command is paused but not terminated.
    Freeze {
        /// ID of the command to freeze
        id: String,
    },
    /// Thaw (resume) a frozen command via SIGCONT.
    Thaw {
        /// ID of the command to thaw
        id: String,
    },
    /// Manage named certificates for per-command access control
    Cert {
        #[command(subcommand)]
        action: CertAction,
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
    pub fn apply_overrides(&self, cfg: &mut Config) {
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
        // (CLI always adds/overrides config)
        let cli_env = self.parse_env_vars();
        cfg.environment.variables.extend(cli_env);
    }
}
