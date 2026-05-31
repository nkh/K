use anyhow::Result;
use std::sync::Arc;

use vrl::cli::args::Cli;
use vrl::cli::dispatch;
use vrl::instance::registry::InstanceRegistry;
use vrl::interactive::display::{detect_terminal_size, run_display_loop, wait_for_child};
use vrl::ipc::server::spawn_control_server;
use vrl::ipc::socket_path_for_pid;
use vrl::process::manager::CommandManager;

/// Spawn a child command from the CLI positional args, if provided.
/// Returns the spawned command ID, or None if no command was given.
async fn spawn_initial_command(
    cli: &Cli,
    manager: &Arc<CommandManager>,
    cfg: &vrl::config::schema::Config,
) -> Result<Option<String>> {
    let cmd_args = match &cli.cmd_args {
        Some(args) if !args.is_empty() => args,
        _ => return Ok(None),
    };

    let cmd = cmd_args[0].clone();
    let args = cmd_args[1..].to_vec();

    // Build per-command exit configuration from CLI flags.
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

    // Send initial keystrokes if --send-keys was specified.
    if let Some(ref keys) = cli.send_keys {
        if let Err(e) = manager.send_keys(&id, keys).await {
            tracing::warn!(error = %e, "Failed to send initial keys");
        } else {
            tracing::info!(keys = %keys, "Sent initial keystrokes");
        }
    }

    Ok(Some(id))
}

/// Detect the terminal size and apply it to the VTTY config when
/// --display is enabled.  CLI flags --vtty-rows / --vtty-cols take
/// precedence over detection.
fn apply_detected_terminal_size(cli: &Cli, cfg: &mut vrl::config::schema::Config) {
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

/// Signal handler for graceful shutdown.
/// Sends on the shutdown channel when SIGINT/SIGTERM is received.
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
                _ = shutdown_rx.recv() => {
                    // Already shutting down from another source
                }
            }
        }
        #[cfg(not(unix))]
        {
            let _ = tokio::signal::ctrl_c().await;
            let _ = shutdown_tx.send(());
        }
    });
}

// ── Application entry point ──

async fn async_main(cli: Cli) -> Result<()> {
    // Init tracing
    tracing_subscriber::fmt::init();

    // Log working directory at startup for diagnostics
    if let Ok(cwd) = std::env::current_dir() {
        tracing::info!(cwd = %cwd.display(), "Working directory");
    }

    // Load and merge configuration
    let mut cfg = dispatch::resolve_config(&cli)?;

    // Detect terminal size for display mode (no-op unless --display)
    apply_detected_terminal_size(&cli, &mut cfg);

    // Initialize instance registry
    let registry = InstanceRegistry::new()?;
    registry.register_current(&cfg)?;

    // Initialize command manager
    let manager = Arc::new(CommandManager::new(cfg.clone()));

    // Spawn child command from CLI positional args, if provided.
    let spawned_id = spawn_initial_command(&cli, &manager, &cfg).await?;

    // Create shutdown channel
    let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel::<()>(1);

    // Install signal handler for clean shutdown
    spawn_signal_handler(shutdown_tx.clone());

    // Start UDS control socket for inter-instance IPC
    let pid = std::process::id();
    let control_socket = socket_path_for_pid(pid);
    spawn_control_server(manager.clone(), control_socket, shutdown_tx.subscribe());

    // Ensure control socket directory exists
    if let Some(parent) = socket_path_for_pid(pid).parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    if cfg.display.enabled {
        // ── Display mode ──
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
        // ── Headless mode: wait for the child to exit or shutdown signal ──
        wait_for_child(&manager, id, shutdown_rx).await;
    } else {
        // No command and no display — nothing to do
        tracing::info!("No command specified and no display. Exiting.");
    }

    // Clean up instance registry entry and control socket
    let _ = registry.unregister_current();
    let _ = std::fs::remove_file(socket_path_for_pid(std::process::id()));

    Ok(())
}

fn main() -> Result<()> {
    // Phase 1: Synchronous pre-runtime (no tokio threads yet)
    let cli = match dispatch::pre_runtime()? {
        Some(cli) => cli,
        None => return Ok(()), // Subcommand handled, exit
    };

    // IPC commands need a minimal tokio runtime for the UDS client.
    // Route them directly without starting a full vrl instance.
    if dispatch::is_ipc_command(&cli) {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?
            .block_on(dispatch::run_ipc_command(cli))?;
        return Ok(());
    }

    // Daemonize if requested — MUST happen before tokio::runtime is created.
    if cli.daemon {
        #[cfg(unix)]
        {
            let mut cfg = dispatch::resolve_config(&cli)?;

            if !cfg.daemon.enabled {
                cfg.daemon.enabled = true;
            }

            vrl::daemon::unix::daemonize(&cfg)?;
        }
        #[cfg(not(unix))]
        {
            anyhow::bail!("--daemon is only supported on Unix-like systems");
        }
    }

    // Use current_thread runtime — no server means no need for multi-threaded I/O.
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(async_main(cli))
}
