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

    /// Server bind address
    #[arg(long, value_name = "ADDR")]
    pub bind: Option<String>,

    /// Server port
    #[arg(long, value_name = "PORT")]
    pub port: Option<u16>,

    /// Run as a background daemon
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
    /// Apply CLI overrides to the loaded configuration
    pub fn apply_overrides(&self, cfg: &mut Config) {
        if let Some(bind) = &self.bind {
            cfg.server.bind = bind.clone();
        }
        if let Some(port) = self.port {
            cfg.server.port = port;
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
