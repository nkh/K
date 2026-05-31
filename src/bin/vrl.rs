use anyhow::Result;
use std::sync::Arc;

use vrl_core::cli::args::Cli;
use vrl_core::cli::dispatch;
use vrl_core::instance::registry::InstanceRegistry;
use vrl_core::interactive::display::{detect_terminal_size, run_display_loop, wait_for_child};
use vrl_core::ipc::server::spawn_control_server;
use vrl_core::ipc::socket_path_for_pid;
use vrl_core::process::manager::CommandManager;

/// Spawn a child command from the CLI positional args, if provided.
async fn spawn_initial_command(
    cli: &Cli,
    manager: &Arc<CommandManager>,
    cfg: &vrl_core::config::schema::Config,
) -> Result<Option<String>> {
    let cmd_args = match &cli.cmd_args {
        Some(args) if !args.is_empty() => args,
        _ => return Ok(None),
    };

    let cmd = cmd_args[0].clone();
    let args = cmd_args[1..].to_vec();

    let per_command_exit = if cli.retain_on_exit
        || cli.snapshot_on_exit.is_some()
        || cli.on_exit.is_some()
        || cli.on_error.is_some()
        || cli.exit_timeout.is_some()
    {
        let mut ec = cfg.default_exit.exit.clone();
        if cli.retain_on_exit {
            ec.retain_on_exit = true;
        }
        if let Some(ref path) = cli.snapshot_on_exit {
            ec.snapshot_on_exit = Some(path.clone());
        }
        Some(ec)
    } else {
        None
    };

    let id = manager
        .spawn(
            cmd,
            args,
            None,
            per_command_exit,
            cfg.environment.variables.clone(),
            None,
            None,
            cli.working_directory.clone(),
        )
        .await?;

    if let Some(ref keys) = cli.send_keys {
        if let Err(e) = manager.send_keys(&id, keys).await {
            tracing::warn!(error = %e, "Failed to send initial keys");
        } else {
            tracing::info!(keys = %keys, "Sent initial keystrokes");
        }
    }

    Ok(Some(id))
}

fn apply_detected_terminal_size(cli: &Cli, cfg: &mut vrl_core::config::schema::Config) {
    if !cfg.display.enabled {
        return;
    }
    let detected = detect_terminal_size();
    if let Some((rows, cols)) = detected {
        let effective_rows = if cli.tabs {
            rows.saturating_sub(1)
        } else {
            rows
        };
        tracing::info!(
            rows,
            cols,
            effective_rows,
            tabs = cli.tabs,
            method = "multi",
            "Detected terminal size for display mode"
        );
        if cli.vtty_rows.is_none() {
            cfg.vtty.rows = effective_rows;
        }
        if cli.vtty_cols.is_none() {
            cfg.vtty.cols = cols;
        }
    } else {
        tracing::warn!("Failed to detect terminal size, using config defaults");
    }
}

fn spawn_signal_handler(shutdown_tx: tokio::sync::broadcast::Sender<()>) {
    tokio::spawn(async move {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};
            let mut sigint = match signal(SignalKind::interrupt()) {
                Ok(s) => s,
                Err(_) => return,
            };
            let mut sigterm = match signal(SignalKind::terminate()) {
                Ok(s) => s,
                Err(_) => return,
            };
            let mut shutdown_rx = shutdown_tx.subscribe();
            tokio::select! {
                _ = sigint.recv() => {
                    tracing::info!("Received SIGINT, exiting");
                    let _ = shutdown_tx.send(());
                }
                _ = sigterm.recv() => {
                    tracing::info!("Received SIGTERM, exiting");
                    let _ = shutdown_tx.send(());
                }
                _ = shutdown_rx.recv() => {}
            }
        }
        #[cfg(not(unix))]
        {
            let _ = tokio::signal::ctrl_c().await;
            let _ = shutdown_tx.send(());
        }
    });
}

async fn async_main(cli: Cli) -> Result<()> {
    tracing_subscriber::fmt::init();

    if let Ok(cwd) = std::env::current_dir() {
        tracing::info!(cwd = %cwd.display(), "Working directory");
    }

    let mut cfg = dispatch::resolve_config(&cli)?;
    apply_detected_terminal_size(&cli, &mut cfg);

    let registry = InstanceRegistry::new()?;
    registry.register_current(&cfg)?;

    let manager = Arc::new(CommandManager::new(cfg.clone()));

    let spawned_id = spawn_initial_command(&cli, &manager, &cfg).await?;

    let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel::<()>(1);
    spawn_signal_handler(shutdown_tx.clone());

    let pid = std::process::id();
    let control_socket = socket_path_for_pid(pid);
    spawn_control_server(manager.clone(), control_socket, shutdown_tx.subscribe());

    if let Some(parent) = socket_path_for_pid(pid).parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    if cfg.display.enabled {
        let log_entries = manager.logger().memory_buffer_arc();
        let effective_display_all = cfg.display.display_all || cfg.interactive.tabs;
        run_display_loop(
            &manager,
            spawned_id.as_deref(),
            cfg.display.refresh_ms,
            effective_display_all,
            shutdown_tx.clone(),
            &cfg.interactive.keybindings,
            &log_entries,
            cfg.interactive.tabs,
        )
        .await;
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
            vrl_core::daemon::unix::daemonize(&cfg)?;
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
