//! Shared startup helpers used by both `vrc` and `vrw` binary entry points.
//!
//! These functions were previously copy-pasted verbatim in `src/bin/vrc.rs` and
//! `src/bin/vrw.rs`.  Extracting them here eliminates the duplication and
//! ensures any future changes are made in exactly one place.

use std::sync::Arc;

use anyhow::Result;

use crate::cli::args::Cli;
use crate::config::schema::Config;
use crate::interactive::display::detect_terminal_size;
use crate::process::manager::CommandManager;

/// Spawn a child command from the CLI positional args, if provided.
///
/// Parses `cli.cmd_args`, builds optional per-command exit configuration from
/// CLI flags, calls `manager.spawn()`, and optionally sends initial keystrokes.
pub async fn spawn_initial_command(
    cli: &Cli,
    manager: &Arc<CommandManager>,
    cfg: &Config,
) -> Result<Option<String>> {
    let cmd_args = match &cli.cmd_args {
        Some(args) if !args.is_empty() => args,
        _ => return Ok(None),
    };

    let cmd = cmd_args[0].clone();
    let cmd_display = cmd.clone();
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

    tracing::info!("spawned command '{}' (PID {})", cmd_display, id);

    if let Some(ref keys) = cli.send_keys {
        if let Err(e) = manager.send_keys(&id, keys).await {
            tracing::warn!(error = %e, "Failed to send initial keys");
        } else {
            tracing::info!(keys = %keys, "Sent initial keystrokes");
        }
    }

    Ok(Some(id))
}

/// Detect the terminal size and apply it to the config's VTTY rows/cols.
///
/// Respects `cli.vtty_rows` / `cli.vtty_cols` overrides — only fills in values
/// that were not explicitly set on the command line.  When `cli.tabs` is true,
/// one row is subtracted to make room for the tab bar.
pub fn apply_detected_terminal_size(cli: &Cli, cfg: &mut Config) {
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

/// Spawn an async task that listens for SIGINT / SIGTERM (or Ctrl-C on
/// non-Unix) and forwards the signal by sending on `shutdown_tx`.
///
/// Also subscribes to `shutdown_rx` so the task exits cleanly when shutdown
/// is initiated from *another* source (e.g. the display loop, a child exit,
/// or an API call).  Without this subscription the tokio signal driver can
/// deadlock during `Runtime::drop`, causing the process to hang.
pub fn spawn_signal_handler(shutdown_tx: tokio::sync::broadcast::Sender<()>) {
    let mut shutdown_rx = shutdown_tx.subscribe();
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
            tokio::select! {
                _ = sigint.recv() => {
                    tracing::info!("Received SIGINT, triggering shutdown");
                    let _ = shutdown_tx.send(());
                }
                _ = sigterm.recv() => {
                    tracing::info!("Received SIGTERM, triggering shutdown");
                    let _ = shutdown_tx.send(());
                }
                _ = shutdown_rx.recv() => {}
            }
        }
        #[cfg(not(unix))]
        {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    let _ = shutdown_tx.send(());
                }
                _ = shutdown_rx.recv() => {}
            }
        }
    });
}
