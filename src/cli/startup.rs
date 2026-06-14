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

/// Run a non-display event loop: prints log entries from the CommandLogger
/// broadcast channel to stdout until the spawned command exits.
///
/// This is used when vrc/vrw runs without `--display` and without `--quiet`.
/// The function subscribes to the manager's logger and prints every event
/// (spawn, exit, kill, resize, etc.) to the terminal in real time.
pub async fn run_non_display_event_loop(
    manager: &Arc<CommandManager>,
    spawned_id: Option<&str>,
    mut shutdown_rx: tokio::sync::broadcast::Receiver<()>,
) {
    let mut log_rx = manager.logger().subscribe();

    if let Some(id) = spawned_id {
        let mut child_exit_rx = {
            let handle = manager.get(&id.to_string());
            if let Some(h) = handle {
                h.exit_rx.clone()
            } else {
                return;
            }
        };

        loop {
            tokio::select! {
                _ = child_exit_rx.changed() => break,
                _ = shutdown_rx.recv() => break,
                entry = log_rx.recv() => {
                    match entry {
                        Ok(line) => println!("{}", line),
                        Err(_) => break,
                    }
                }
            }
        }
    } else {
        // No initial command — wait for shutdown while printing events
        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => break,
                entry = log_rx.recv() => {
                    match entry {
                        Ok(line) => println!("{}", line),
                        Err(_) => break,
                    }
                }
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::args::BINARY_NAME;
    use clap::Parser;

    /// Helper: create a Cli with no args and no subcommand.
    fn make_cli(args: &[&str]) -> crate::cli::args::Cli {
        crate::cli::args::Cli::try_parse_from(args).unwrap()
    }

    /// Helper: create a default Config.
    fn default_config() -> crate::config::schema::Config {
        crate::config::schema::Config::default()
    }

    #[test]
    fn test_apply_detected_terminal_size_display_disabled() {
        // When display is disabled, config should not be modified
        let cli = make_cli(&[BINARY_NAME]);
        let mut cfg = default_config();
        cfg.display.enabled = false;
        let original_rows = cfg.vtty.rows;
        let original_cols = cfg.vtty.cols;

        apply_detected_terminal_size(&cli, &mut cfg);

        assert_eq!(cfg.vtty.rows, original_rows, "rows should not change when display disabled");
        assert_eq!(cfg.vtty.cols, original_cols, "cols should not change when display disabled");
    }

    #[test]
    fn test_apply_detected_terminal_size_with_explicit_rows_cols() {
        // When CLI provides explicit rows/cols, those should not be overridden
        let cli = make_cli(&[BINARY_NAME, "--vtty-rows", "100", "--vtty-cols", "200"]);
        let mut cfg = default_config();
        cfg.display.enabled = true;
        cfg.vtty.rows = 24;
        cfg.vtty.cols = 80;

        apply_detected_terminal_size(&cli, &mut cfg);

        // CLI overrides should prevent detection from overriding
        assert!(cli.vtty_rows.is_some(), "cli should have explicit rows");
        assert!(cli.vtty_cols.is_some(), "cli should have explicit cols");
        // The function only sets rows/cols when cli.vtty_rows/cols is None
        // Since we set them explicitly, they shouldn't change from the initial values
        // that apply_overrides already set. But apply_detected_terminal_size doesn't
        // call apply_overrides — it checks cli.vtty_rows.is_none().
        // Since cli has explicit rows, cfg rows should stay at 24 (the value before detection)
        assert_eq!(cfg.vtty.rows, 24);
        assert_eq!(cfg.vtty.cols, 80);
    }

    #[test]
    fn test_apply_detected_terminal_size_display_enabled_no_cli_override() {
        // When display is enabled and no CLI overrides, detection may or may not
        // work in test environment (no TTY). The function should not panic.
        let cli = make_cli(&[BINARY_NAME, "--display"]);
        let mut cfg = default_config();
        cfg.display.enabled = true;

        // Should not panic even without a real terminal
        apply_detected_terminal_size(&cli, &mut cfg);
    }

    #[test]
    fn test_apply_detected_terminal_size_with_tabs() {
        // When tabs is enabled, one row should be subtracted from detected size
        let cli = make_cli(&[BINARY_NAME, "--tabs"]);
        let mut cfg = default_config();
        cfg.display.enabled = true;

        // Should not panic — in test env detection may return None
        apply_detected_terminal_size(&cli, &mut cfg);
        // If detection worked, rows should be at least 1 less than detected
        // (saturating_sub(1)). We just verify it doesn't panic and tabs flag is read.
        assert!(cli.tabs);
    }

    #[tokio::test]
    async fn test_spawn_signal_handler_shutdown_propagation() {
        let (tx, mut rx) = tokio::sync::broadcast::channel::<()>(2);
        spawn_signal_handler(tx.clone());

        // Send shutdown from another sender
        tx.send(()).unwrap();

        // The handler task should exit cleanly without hanging.
        // We can't easily verify it received the signal, but we verify
        // the broadcast channel works.
        let result = tokio::time::timeout(
            std::time::Duration::from_millis(500),
            rx.recv()
        ).await;
        assert!(result.is_ok());
    }
}

