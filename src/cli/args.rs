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

    /// Run as a background daemon (Unix only)
    #[arg(long)]
    pub daemon: bool,

    /// Show VTTY on local terminal screen
    #[arg(long)]
    pub display: bool,

    /// Disable local terminal display
    #[arg(long)]
    pub no_display: bool,

    /// Log API commands to terminal
    #[arg(long)]
    pub log: bool,

    /// Log API commands to file
    #[arg(long, value_name = "FILE")]
    pub log_file: Option<String>,

    /// VTTY rows
    #[arg(long, value_name = "N")]
    pub vtty_rows: Option<u16>,

    /// VTTY columns
    #[arg(long, value_name = "N")]
    pub vtty_cols: Option<u16>,

    /// Subcommand (list, stop)
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
}

impl Cli {
    /// Apply CLI overrides to the loaded configuration.
    /// CLI flags take the highest precedence (override global and local config).
    pub fn apply_overrides(&self, cfg: &mut Config) {
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
        if self.display {
            cfg.display.enabled = true;
        }
        if self.no_display {
            cfg.display.enabled = false;
        }
        if self.daemon {
            cfg.daemon.enabled = true;
            cfg.display.enabled = false;
        }
        if self.log {
            cfg.command_log.enabled = true;
        }
        if let Some(file) = &self.log_file {
            cfg.command_log.enabled = true;
            cfg.command_log.file = Some(file.clone());
        }
        if let Some(rows) = self.vtty_rows {
            cfg.vtty.rows = rows;
        }
        if let Some(cols) = self.vtty_cols {
            cfg.vtty.cols = cols;
        }
    }
}
