use anyhow::Result;
use clap::Parser;
use std::sync::Arc;
use tokio::sync::broadcast;

mod cli;
mod config;
mod daemon;
mod handles;
mod instance;
mod logging;
mod process;
mod vtty;
mod web;

use cli::args::{Cli, Commands};
use config::loader::load_config;
use instance::registry::InstanceRegistry;
use process::manager::CommandManager;
use web::server::start_server;


#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    // Handle subcommands first (list, stop)
    match cli.command {
        Some(Commands::List) => {
            let registry = InstanceRegistry::new()?;
            registry.print_list();
            return Ok(());
        }
        Some(Commands::Stop { pid }) => {
            let registry = InstanceRegistry::new()?;
            registry.stop_instance(pid).await?;
            return Ok(());
        }
        None => {}
    }

    // Load and merge configuration
    let mut cfg = load_config(cli.config.as_deref())?;
    cli.apply_overrides(&mut cfg);

    // Daemonize if requested (Unix only) — must happen BEFORE tokio runtime
    // starts any significant work (signal handlers, etc.). See the dedicated
    // daemonize fix for the full architectural solution.
    if cfg.daemon.enabled {
        #[cfg(unix)]
        {
            daemon::unix::daemonize(&cfg)?;
        }
        #[cfg(not(unix))]
        {
            anyhow::bail!("--daemon is only supported on Unix-like systems");
        }
    }

    // Initialize instance registry
    let registry = InstanceRegistry::new()?;
    registry.register_current(&cfg)?;

    // Initialize command manager
    let manager = Arc::new(CommandManager::new(cfg.clone()));

    // If a child command was provided, spawn it immediately
    if let Some(cmd_args) = cli.cmd_args {
        if !cmd_args.is_empty() {
            let cmd = cmd_args[0].clone();
            let args = cmd_args[1..].to_vec();
            let _id = manager.spawn(cmd, args).await?;
        }
    }

    // Create shutdown channel — passed explicitly, no globals
    let (shutdown_tx, _shutdown_rx) = broadcast::channel::<()>(1);

    // Start the web server
    let server_handle = tokio::spawn(async move {
        start_server(cfg.server.bind, cfg.server.port, manager.clone(), shutdown_tx).await
    });

    // Wait for server to finish
    let _ = server_handle.await?;

    // Cleanup on exit
    registry.unregister_current()?;

    Ok(())
}
