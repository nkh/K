use anyhow::Result;
use std::sync::Arc;
use tokio::sync::broadcast;

use vrunner::cli::args::{Cli, Commands};
use vrunner::cli::subcommands;
use vrunner::config::loader::load_config;
use vrunner::config::merge::apply_profile;
use vrunner::config::schema::Config;
use vrunner::config::validation::{validate_config, ValidationLevel};
use vrunner::daemon;
use vrunner::instance::registry::InstanceRegistry;
use vrunner::interactive::display::{detect_terminal_size, run_display_loop, wait_for_child};
use vrunner::process::manager::CommandManager;
use vrunner::web::auth::AuthManager;
use vrunner::web::server::start_server;

/// Load, profile-merge, and CLI-override the configuration.
///
/// Config loading is intentionally **lazy** — it only runs after clap has
/// finished parsing, so `vrunner --help` and `vrunner --version` respond
/// instantly without touching the filesystem.  Subcommands that don't
/// need config (list, stop, spawn, freeze, thaw, etc.) also return
/// before reaching this code path.
fn resolve_config(cli: &Cli) -> Result<Config> {
    let mut cfg = load_config(cli.config.as_deref())?;

    // Apply named profile if specified
    if let Some(ref profile_name) = cli.profile {
        if let Some(profile) = cfg.profiles.entries.clone().get(profile_name) {
            tracing::info!(profile = %profile_name, "Applying configuration profile");
            cfg = apply_profile(cfg, profile);
        } else {
            anyhow::bail!(
                "Profile '{}' not found. Available profiles: {}",
                profile_name,
                if cfg.profiles.entries.is_empty() {
                    "(none defined in config)".to_string()
                } else {
                    cfg.profiles
                        .entries
                        .keys()
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(", ")
                }
            );
        }
    }

    // Apply CLI overrides (highest precedence)
    cli.apply_overrides(&mut cfg)?;

    // Validate the final merged configuration
    let issues = validate_config(&cfg);
    let mut error_fields = Vec::new();
    for issue in &issues {
        match issue.level {
            ValidationLevel::Error => {
                tracing::error!(field = %issue.field, "{}", issue.message);
                error_fields.push(issue.field.clone());
            }
            ValidationLevel::Warning => {
                tracing::warn!(field = %issue.field, "{}", issue.message);
            }
        }
    }
    if !error_fields.is_empty() {
        anyhow::bail!(
            "Configuration validation failed ({})
Fields with errors: {}",
            error_fields.len(),
            error_fields.join(", "),
        );
    }

    Ok(cfg)
}

/// Synchronous pre-runtime phase: parse CLI, handle subcommands, load config,
/// and daemonize. Daemonization MUST happen before the tokio runtime starts,
/// because fork() only copies the calling thread while tokio's multi-threaded
/// runtime creates internal threads for I/O, timers, and blocking tasks.
fn pre_runtime() -> Result<Option<Cli>> {
    let cli = Cli::parse_with_version();

    // Handle subcommands that don't need the runtime
    match &cli.command {
        Some(Commands::List) => {
            // list is async (needs to query instances for their commands), fall through
        }
        Some(Commands::Stop { pid: _ }) => {
            // stop_instance is async (uses reqwest), so we need the runtime
            // Fall through to the async phase
        }
        Some(Commands::Spawn { .. }) => {
            // spawn is async (uses reqwest), fall through to async phase
        }
        Some(Commands::Freeze { pid: _ }) => {
            // freeze is async (uses reqwest), fall through to async phase
        }
        Some(Commands::Thaw { pid: _ }) => {
            // thaw is async (uses reqwest), fall through to async phase
        }
        Some(Commands::Cert { action }) => {
            // Cert subcommands are synchronous — handle them here
            subcommands::handle_cert_command(action)?;
            return Ok(None);
        }
        Some(Commands::ListVrunner) => {
            // list-vrunner is async (needs HTTP), fall through to async phase
        }
        Some(Commands::ListCommands) => {
            // list-commands is async (needs HTTP), fall through to async phase
        }
        Some(Commands::StopCommand { target: _ }) => {
            // stop-command is async (needs HTTP), fall through to async phase
        }
        Some(Commands::Purge { target: _ }) => {
            // purge is async (needs HTTP), fall through to async phase
        }
        Some(Commands::Resize { .. }) => {
            // resize is async (needs HTTP), fall through to async phase
        }
        Some(Commands::ConfigCheck) => {
            subcommands::handle_config_check_command(cli.config.as_deref())?;
            return Ok(None);
        }
        None => {}
    }

    Ok(Some(cli))
}

/// Dispatch async subcommands.  Returns `Ok(true)` if a subcommand was
/// handled (the caller should return), or `Ok(false)` if no subcommand
/// matched (the caller should continue to the main server path).
async fn handle_subcommands(cli: &Cli) -> Result<bool> {
    match &cli.command {
        Some(Commands::List) => {
            subcommands::handle_list_command(cli).await?;
            Ok(true)
        }
        Some(Commands::Stop { pid }) => {
            let registry = InstanceRegistry::new()?;
            let instances = registry.list_instances();
            let pid = subcommands::resolve_stop_target(*pid, &instances);
            tracing::info!(instance_pid = pid, "Stopping vrunner instance");
            registry.stop_instance(pid).await?;
            Ok(true)
        }
        Some(Commands::Spawn {
            cmd,
            args,
            rows,
            cols,
        }) => {
            subcommands::handle_spawn_command(cli, cmd, args, *rows, *cols).await?;
            Ok(true)
        }
        Some(Commands::Freeze { pid }) => {
            subcommands::handle_freeze_command(cli, *pid).await?;
            Ok(true)
        }
        Some(Commands::Thaw { pid }) => {
            subcommands::handle_thaw_command(cli, *pid).await?;
            Ok(true)
        }
        Some(Commands::ListVrunner) => {
            subcommands::handle_list_vrunner_command(cli).await?;
            Ok(true)
        }
        Some(Commands::ListCommands) => {
            subcommands::handle_list_commands_command(cli).await?;
            Ok(true)
        }
        Some(Commands::StopCommand { target }) => {
            let stopped = subcommands::handle_stop_command(cli, target.as_deref()).await?;
            if !stopped {
                match target {
                    Some(t) => tracing::error!(
                        "No matching command found for '{}'. Use `vrunner list` to see running commands.", t
                    ),
                    None => tracing::error!(
                        "No command to stop. Use `vrunner list` to see running commands."
                    ),
                }
                std::process::exit(1);
            }
            Ok(true)
        }
        Some(Commands::Purge { target }) => {
            let purged = subcommands::handle_purge_command(cli, target.as_deref()).await?;
            if !purged {
                match target {
                    Some(t) => tracing::error!(
                        "No matching exited command found for '{}'. Use `vrunner list` to see commands.", t
                    ),
                    None => tracing::error!(
                        "No exited command to purge. Use `vrunner list` to see commands."
                    ),
                }
                std::process::exit(1);
            }
            Ok(true)
        }
        Some(Commands::Resize { target, rows, cols }) => {
            subcommands::handle_resize_command(cli, target, *rows, *cols).await?;
            Ok(true)
        }
        // Cert and ConfigCheck are handled synchronously in pre_runtime()
        // and never reach the async phase.
        Some(Commands::Cert { .. }) | Some(Commands::ConfigCheck) => unreachable!(),
        None => Ok(false),
    }
}

/// Detect the terminal size and apply it to the VTTY config when
/// --display is enabled.  CLI flags --vtty-rows / --vtty-cols take
/// precedence over detection.
fn apply_detected_terminal_size(cli: &Cli, cfg: &mut Config) {
    if !cfg.display.enabled {
        return;
    }
    let detected = detect_terminal_size();
    if let Some((rows, cols)) = detected {
        // When the tab bar is shown, subtract 1 row so the VTTY
        // content fits in the remaining lines.  Without this the
        // last line of terminal output (e.g. btop status bar,
        // vim status line) is clipped.
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

/// Spawn a child command from the CLI positional args, if provided.
/// Returns the spawned command ID, or None if no command was given.
async fn spawn_initial_command(
    cli: &Cli,
    manager: &Arc<CommandManager>,
    cfg: &Config,
) -> Result<Option<String>> {
    let cmd_args = match &cli.cmd_args {
        Some(args) if !args.is_empty() => args,
        _ => return Ok(None),
    };

    let cmd = cmd_args[0].clone();
    let args = cmd_args[1..].to_vec();

    // Build per-command exit configuration from CLI flags.
    // These override the global config defaults for this command only.
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
        // on_exit, on_error, and exit_timeout are already applied to
        // the global default by apply_overrides, so they'll be
        // inherited. But we still build an explicit Some() so the
        // per-command path is taken consistently.
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
            None,
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

// ── Application entry point ──

// Application entry point: parses args, loads config, sets up
// manager/spawner, then either starts web server, display loop,
// or runs a single command based on CLI flags.
async fn async_main(cli: Cli) -> Result<()> {
    // Initialize tracing (after daemonize, so logs go to the right place)
    tracing_subscriber::fmt::init();

    // Dispatch async subcommands (list, stop, spawn, etc.)
    if handle_subcommands(&cli).await? {
        return Ok(());
    }

    // Load and merge configuration (lazy — only reached if no subcommand handled)
    let mut cfg = resolve_config(&cli)?;

    // Detect terminal size for display mode (no-op unless --display)
    apply_detected_terminal_size(&cli, &mut cfg);

    // Initialize instance registry
    let registry = InstanceRegistry::new()?;
    registry.register_current(&cfg)?;

    // Load or generate auth token if auth is required
    let auth_token = if cfg.security.require_auth {
        Some(AuthManager::load_or_generate(&cfg.security.token_file)?)
    } else {
        None
    };

    // Initialize command manager
    let manager = Arc::new(CommandManager::new(cfg.clone()));

    // Spawn child command from CLI positional args, if provided.
    let spawned_id = spawn_initial_command(&cli, &manager, &cfg).await?;

    // Create shutdown channel — passed explicitly, no globals
    let (shutdown_tx, _shutdown_rx) = broadcast::channel::<()>(1);

    // Start the web server
    let server_handle = tokio::spawn({
        let manager = manager.clone();
        let shutdown_tx = shutdown_tx.clone();
        let cfg = cfg.clone();
        async move {
            start_server(
                cfg.server.bind.clone(),
                cfg.server.port,
                manager.clone(),
                shutdown_tx,
                auth_token,
                cfg.tls.enabled,
                cfg.tls.cert_file.as_deref(),
                cfg.tls.key_file.as_deref(),
                &cfg,
            )
            .await
        }
    });

    // Brief pause to let the server bind before we enter the display loop.
    // If the server fails to start, the server_handle will resolve with an
    // error which we propagate after the loop.
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    if cfg.display.enabled {
        // ── Display mode ──
        // Run an inline display loop (like mprocs).  The loop renders the
        // active command's VTTY buffer directly to the local terminal,
        // forwards keystrokes to the child, and exits on Ctrl+\ or child
        // death (when a direct child was spawned).
        let log_entries = manager.logger().memory_buffer_arc();
        // When tabs are enabled, treat as display_all so the display
        // stays active and switches between tab commands instead of
        // showing "primary command exited".
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
        // run_display_loop always returns true (shutdown triggered).
        // The dismissed path now waits for q/Ctrl+C inside the loop.
    } else if let Some(ref id) = spawned_id {
        // ── Headless mode with direct child ──
        // No display, but a child was spawned directly via CLI.  Wait for
        // the child process to exit, then decide whether to shut down.
        wait_for_child(&manager, id).await;
        // Policy: only shut down if this was the last running command.
        if manager.list().is_empty() {
            let _ = shutdown_tx.send(());
        } else {
            // Other commands remain; transition to idle server mode.
            let mut rx = shutdown_tx.subscribe();
            let _ = rx.recv().await;
        }
    } else {
        // ── Idle server mode ──
        // No display, no direct child.  Wait for an external shutdown
        // signal (SIGINT, SIGTERM, or the /api/shutdown endpoint).
        let mut rx = shutdown_tx.subscribe();
        let _ = rx.recv().await;
    }

    // Wait for the server to finish (up to 3 seconds) then exit.
    //
    // IMPORTANT: we always use std::process::exit(0) instead of returning
    // Ok(()).  Returning causes block_on() to return, which drops the
    // tokio Runtime.  During Runtime::drop, tokio cleans up its signal
    // drivers — this DEADLOCKS because spawn_signal_handler installed
    // tokio's signal driver for SIGINT/SIGTERM, and the cleanup conflicts
    // with the driver's internal lock (documented at server.rs:103).
    //
    // process::exit(0) calls _exit() which terminates the process
    // immediately, skipping all destructors including Runtime::drop.
    // The OS reaps child processes, closes file descriptors, and
    // releases all resources — no cleanup is needed.
    tokio::select! {
        _ = server_handle => {
            tracing::info!("Server shut down");
        }
        _ = tokio::time::sleep(std::time::Duration::from_secs(3)) => {
            tracing::warn!("Server did not shut down within 3s, forcing exit");
        }
    }

    std::process::exit(0);
}

fn main() -> Result<()> {
    // Phase 1: Synchronous pre-runtime (no tokio threads yet)
    let cli = match pre_runtime()? {
        Some(cli) => cli,
        None => return Ok(()), // Subcommand handled, exit
    };

    // Daemonize if requested — MUST happen before tokio::runtime is created.
    // At this point, only the main thread exists, so fork() is safe.
    // After daemonization, the original process exits and the daemon
    // (grandchild of fork) continues as the new process.
    if cli.daemon {
        #[cfg(unix)]
        {
            // For daemon mode, we need to load config early to get log file paths.
            let mut cfg = resolve_config(&cli)?;

            if !cfg.daemon.enabled {
                // CLI --daemon flag overrides config
                cfg.daemon.enabled = true;
            }

            daemon::unix::daemonize(&cfg)?;
            // After daemonize(), we are the daemon process.
            // Only the main thread exists — safe to start tokio now.
        }
        #[cfg(not(unix))]
        {
            anyhow::bail!("--daemon is only supported on Unix-like systems");
        }
    }

    // Phase 2: Start tokio runtime and run async main
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(async_main(cli))
}
