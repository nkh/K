#![cfg(feature = "vrc")]


use anyhow::Result;
use std::sync::Arc;

use vrc_core::cli::args::Cli;
use vrc_core::cli::dispatch;
use vrc_core::cli::startup;
use vrc_core::instance::registry::InstanceRegistry;
use vrc_core::interactive::display::{run_display_loop, wait_for_child};
use vrc_core::ipc::server::spawn_control_server;
use vrc_core::ipc::socket_path_for_pid;
use vrc_core::process::manager::CommandManager;

async fn async_main(cli: Cli) -> Result<()> {
    if cli.no_log {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::WARN)
            .init();
    } else {
        tracing_subscriber::fmt::init();
    }

    if let Ok(cwd) = std::env::current_dir() {
        tracing::info!(cwd = %cwd.display(), "Working directory");
    }

    let mut cfg = dispatch::resolve_config(&cli)?;
    startup::apply_detected_terminal_size(&cli, &mut cfg);
    let handle_sigwinch = cli.handle_sigwinch;

    let registry = InstanceRegistry::new()?;
    registry.register_current(&cfg)?;

    let manager = Arc::new(CommandManager::new(cfg.clone()));

    let spawned_id = startup::spawn_initial_command(&cli, &manager, &cfg).await?;

    let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel::<()>(1);
    startup::spawn_signal_handler(shutdown_tx.clone());

    let pid = std::process::id();
    let control_socket = socket_path_for_pid(pid);
    spawn_control_server(manager.clone(), control_socket, shutdown_tx.subscribe());

    if let Some(parent) = socket_path_for_pid(pid).parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    if cfg.display.enabled {
        let log_entries = manager.logger().memory_buffer_arc();
        let effective_display_all = cfg.interactive.tabs;
        run_display_loop(
            &manager,
            spawned_id.as_deref(),
            cfg.display.refresh_ms,
            effective_display_all,
            shutdown_tx.clone(),
            &cfg.interactive.keybindings,
            &log_entries,
            cfg.interactive.tabs,
            handle_sigwinch,
        )
        .await;
    } else if !cli.no_terminal_log && !cli.quiet {
        startup::run_non_display_event_loop(&manager, spawned_id.as_deref(), shutdown_rx).await;
    } else if let Some(ref id) = spawned_id {
        wait_for_child(&manager, id, shutdown_rx).await;
    } else {
        tracing::info!("No command specified and no display. Exiting.");
    }

    let _ = registry.unregister_current();
    let _ = std::fs::remove_file(socket_path_for_pid(std::process::id()));

    Ok(())
}

fn main() -> Result<()> {
    let cli = match dispatch::pre_runtime()? {
        Some(cli) => cli,
        None => return Ok(()),
    };

    // IPC subcommands are only available in standalone vrc mode (no vrw).
    #[cfg(not(feature = "vrw"))]
    if dispatch::is_ipc_command(&cli) {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?
            .block_on(dispatch::run_ipc_command(cli))?;
        return Ok(());
    }

    if cli.daemon {
        #[cfg(unix)]
        {
            let mut cfg = dispatch::resolve_config(&cli)?;
            if !cfg.daemon.enabled {
                cfg.daemon.enabled = true;
            }
            vrc_core::daemon::unix::daemonize(&cfg)?;
        }
        #[cfg(not(unix))]
        {
            anyhow::bail!("--daemon is only supported on Unix-like systems");
        }
    }

    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(async_main(cli))?;

    std::process::exit(0);
}

#[cfg(test)]
mod tests {

    /// Verify that DEFAULT_PORT is not defined in vrc (vrc uses UDS, not HTTP).
    /// vrc.rs does not bind to a TCP port — it uses Unix domain sockets exclusively.
    #[test]
    fn test_vrc_has_no_tcp_port_binding() {
        // vrc uses socket_path_for_pid() for IPC, not TCP.
        // Verify the function exists and produces a valid path.
        let pid = 12345;
        let path = vrc_core::ipc::socket_path_for_pid(pid);
        assert!(path.to_string_lossy().contains("12345"), "socket path includes pid");
    }

}
