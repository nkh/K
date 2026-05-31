use anyhow::Result;
use std::sync::Arc;
use tokio::sync::broadcast;

use vrunner::cli::args::Cli;
use vrunner::cli::dispatch;
use vrunner::instance::registry::InstanceRegistry;
use vrunner::interactive::display::{detect_terminal_size, run_display_loop, wait_for_child};
use vrunner::process::manager::CommandManager;
use vrunner::web::auth::AuthManager;
use vrunner::web::server::start_server;

/// Default port for client-mode discovery.
const DEFAULT_PORT: u16 = 9090;

// ── Optimization #6: Fast PID check without sysinfo ──
/// Check whether a process with the given PID is alive and its executable
/// name contains "vrunner".  Uses direct `/proc/<pid>/exe` readlink on Linux
/// instead of `sysinfo::System::new_all()` which scans ALL processes.
///
/// Returns `true` if the PID file points to a live vrunner process.
#[allow(dead_code)]
fn is_vrunner_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        // First check: does /proc/<pid> exist at all?
        let proc_path = std::path::Path::new("/proc").join(pid.to_string());
        if !proc_path.exists() {
            return false;
        }

        // Second check: read /proc/<pid>/comm to verify it's vrunner.
        // This is a single small read — much cheaper than scanning all PIDs.
        let comm_path = proc_path.join("comm");
        if let Ok(comm) = std::fs::read_to_string(&comm_path) {
            if comm.trim().to_lowercase().contains("vrunner") {
                return true;
            }
            // PID recycled to another process — caller should clean up
            return false;
        }

        // Fallback: check if the process is at least running
        // (may not have /proc on all Unix, but we tried)
        proc_path.exists()
    }
    #[cfg(not(unix))]
    {
        // Non-Unix fallback: use kill(pid, 0) to check liveness
        // No name verification possible without sysinfo
        unsafe { libc::kill(pid as i32, 0) == 0 }
    }
}

/// Fast check: does a PID file for a vrunner instance on the given port exist
/// and point to a live process?  This avoids the cost of building a reqwest
/// client and attempting a TCP+HTTP connection when no instance is running.
///
/// Optimization #6: Uses direct /proc/<pid>/comm check instead of
/// `sysinfo::System::new_all()` which scans ALL processes on the system.
fn has_live_instance_on_port(bind: &str, port: u16) -> bool {
    let registry = match InstanceRegistry::new() {
        Ok(r) => r,
        Err(_) => return false,
    };
    let instances = registry.list_instances_fast();
    instances
        .iter()
        .any(|info| info.port == port && info.bind == bind)
}

/// Check if a TCP port is available for binding.
/// Returns Ok(()) if the port is free, Err with a descriptive message if not.
fn check_port_available(bind: &str, port: u16) -> Result<()> {
    let addr = format!("{}:{}", bind, port);
    match std::net::TcpListener::bind(&addr) {
        Ok(listener) => {
            // Successfully bound — port is free. Drop the listener immediately.
            drop(listener);
            Ok(())
        }
        Err(e) => {
            anyhow::bail!(
                "Port {} is already in use (bind address: {}). \n\
                 Use `vrunner list` to see running instances, or specify \n\
                 a different port with `--port <PORT>`. \n\
                 Error: {}",
                port, bind, e
            );
        }
    }
}

/// Client mode: try to send the command to a running vrunner instance
/// on the default port. Returns Ok(true) if the command was sent
/// successfully (caller should exit), Ok(false) if no server was
/// found (caller should start a new server), or Err on other failures.
///
/// Optimized (#1+3): checks PID files first (filesystem-only, no network I/O),
/// then builds the heavy reqwest client only if a server is confirmed alive.
async fn try_client_mode(cli: &Cli) -> Result<bool> {
    let cmd_args = match &cli.cmd_args {
        Some(args) if !args.is_empty() => args,
        _ => return Ok(false),
    };

    let cmd = &cmd_args[0];
    let args = &cmd_args[1..];

    // Determine the bind address to probe (use config if available, else default)
    let bind = cli.bind.clone().unwrap_or_else(|| "127.0.0.1".to_string());

    // ── Optimization #3: Fast path — check PID files before any network I/O ──
    // If no vrunner instance is registered for the default port, skip
    // the TCP probe entirely.  This saves ~5-15ms of reqwest client
    // initialization on the common case (no server running).
    if !has_live_instance_on_port(&bind, DEFAULT_PORT) {
        return Ok(false);
    }

    // ── Slow path: server might be running, send the command via HTTP ──
    let probe_url = format!("http://{}:{}/api/commands", bind, DEFAULT_PORT);

    tracing::info!(
        url = %probe_url,
        cmd = %cmd,
        "No --port specified; trying to send command to running instance"
    );

    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(2))
        .timeout(std::time::Duration::from_secs(5))
        .build();

    let client = match client {
        Ok(c) => c,
        Err(e) => {
            tracing::info!(error = %e, "Failed to build HTTP client for client mode");
            return Ok(false);
        }
    };

    let mut body = serde_json::json!({
        "cmd": cmd,
        "args": args,
    });

    // Add working directory if specified
    if let Some(ref dir) = cli.working_directory {
        body["working_directory"] = serde_json::json!(dir);
    }

    let resp = match client.post(&probe_url).json(&body).send().await {
        Ok(resp) => resp,
        Err(e) => {
            tracing::info!(
                error = %e,
                url = %probe_url,
                "No running vrunner instance found at default port — starting new server"
            );
            return Ok(false);
        }
    };

    let status = resp.status();
    let result: serde_json::Value = match resp.json().await {
        Ok(v) => v,
        Err(e) => {
            tracing::info!(error = %e, "Non-JSON response from instance");
            return Ok(false);
        }
    };

    if status.is_success() {
        let cmd_pid = result["data"]["pid"].as_u64().unwrap_or(0);
        let _cmd_id = result["data"]["id"].as_str().unwrap_or("?");
        println!(
            "Command sent to running vrunner instance on port {}",
            DEFAULT_PORT
        );
        println!("  PID:       {}", cmd_pid);
        println!("  VTTY:      http://{}:{}/admin/{}", bind, DEFAULT_PORT, cmd);
        Ok(true)
    } else {
        let error = result["error"].as_str().unwrap_or("Unknown error");
        tracing::warn!(
            status = %status,
            error = %error,
            "Server responded with error in client mode"
        );
        Ok(false)
    }
}

/// Detect the terminal size and apply it to the VTTY config when
/// --display is enabled.  CLI flags --vtty-rows / --vtty-cols take
/// precedence over detection.
fn apply_detected_terminal_size(cli: &Cli, cfg: &mut vrunner::config::schema::Config) {
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

/// Spawn a child command from the CLI positional args, if provided.
/// Returns the spawned command ID, or None if no command was given.
async fn spawn_initial_command(
    cli: &Cli,
    manager: &Arc<CommandManager>,
    cfg: &vrunner::config::schema::Config,
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

// ── Application entry point ──

async fn async_main(cli: Cli) -> Result<()> {
    // Optimization #4: Defer tracing initialization past subcommand dispatch.
    // Subcommands like `vrunner list` or `vrunner config-check` respond
    // faster without the overhead of setting up a full tracing subscriber.
    // Only init tracing after we know we're in the main server/no-server path.

    // Dispatch async subcommands (list, stop, spawn, etc.)
    if dispatch::handle_subcommands(&cli).await? {
        return Ok(());
    }

    // Now we're committed to running a vrunner instance — init tracing.
    tracing_subscriber::fmt::init();

    // Log working directory at startup for diagnostics
    if let Ok(cwd) = std::env::current_dir() {
        tracing::info!(cwd = %cwd.display(), "Working directory");
    }

    // ── Client mode: send command to running instance if no --port given ──
    // When the user runs `vrunner htop` (no --port), try to forward the
    // command to the existing vrunner instance on the default port.
    // Only fall through to start a new server if no instance is reachable.
    if cli.port.is_none() {
        if try_client_mode(&cli).await? {
            // Command was sent to a running instance — exit successfully.
            return Ok(());
        }
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

    // ── --no-server mode ──
    if cli.no_server {
        return run_no_server_mode(cli, manager, cfg).await;
    }

    // ── Server mode (default) ──

    // Load or generate auth token if auth is required
    let auth_token = if cfg.security.require_auth {
        Some(AuthManager::load_or_generate(&cfg.security.token_file)?)
    } else {
        None
    };

    // ── Port availability check ──
    if let Err(e) = check_port_available(&cfg.server.bind, cfg.server.port) {
        tracing::error!(bind = %cfg.server.bind, port = cfg.server.port, "{}", e);
        anyhow::bail!("{}", e);
    }

    // Spawn child command from CLI positional args, if provided.
    let spawned_id = spawn_initial_command(&cli, &manager, &cfg).await?;

    // Create shutdown channel — passed explicitly, no globals
    let (shutdown_tx, _shutdown_rx) = broadcast::channel::<()>(1);

    // Start the web server (port is guaranteed available after check above)
    let server_handle = tokio::spawn({
        let manager = manager.clone();
        let shutdown_tx = shutdown_tx.clone();
        let cfg = cfg.clone();
        let auth_token = auth_token.clone();
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

    // Optimization #7: The 100ms sleep has been removed.  The port check at
    // line ~280 above guarantees the port is free, so `axum_server::bind()`
    // will succeed.  The display loop and wait_for_child don't need the
    // server to be accepting connections yet — they only need the PTY child
    // process, which is already spawned.

    // Register with a primary vrunner instance if --register-with is set.
    if let Some(register_port) = cli.register_with {
        let my_url = format!("http://{}:{}", cfg.server.bind, cfg.server.port);
        let my_label = format!("vrunner:{}", std::process::id());
        let my_token = auth_token.clone().unwrap_or_default();
        let primary_url = format!("http://{}:{}/api/peers", cfg.server.bind, register_port);
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            let body = serde_json::json!({
                "url": my_url,
                "label": my_label,
                "token": my_token,
            });
            let client = reqwest::Client::builder()
                .connect_timeout(std::time::Duration::from_secs(3))
                .timeout(std::time::Duration::from_secs(5))
                .build();
            match client {
                Ok(c) => match c.post(&primary_url).json(&body).send().await {
                    Ok(resp) if resp.status().is_success() => {
                        tracing::info!(
                            primary = %primary_url,
                            my_url = %my_url,
                            "Registered with primary instance"
                        );
                    }
                    Ok(resp) => {
                        tracing::warn!(
                            primary = %primary_url,
                            status = %resp.status(),
                            "Failed to register with primary instance (HTTP error)"
                        );
                    }
                    Err(e) => {
                        tracing::warn!(
                            primary = %primary_url,
                            error = %e,
                            "Failed to register with primary instance (connection error)"
                        );
                    }
                },
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to build HTTP client for registration");
                }
            }
        });
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
        // ── Headless mode with direct child ──
        wait_for_child(&manager, id).await;
        // Do NOT auto-shutdown when the last command exits — the web UI
        // and API are still active and can spawn new commands.  Wait for
        // an explicit shutdown signal (SIGINT, SIGTERM, /api/shutdown).
        let mut rx = shutdown_tx.subscribe();
        let _ = rx.recv().await;
    } else {
        // ── Idle server mode ──
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

/// ── --no-server mode: run without web server ──
///
/// Spawns the command, optionally displays the VTTY, and waits for the child
/// to exit.  No web server, no API, no auth, no TLS — just PTY + display.
async fn run_no_server_mode(
    cli: Cli,
    manager: Arc<CommandManager>,
    cfg: vrunner::config::schema::Config,
) -> Result<()> {
    tracing::info!("Running in --no-server mode (no web server, no API)");

    // Spawn child command from CLI positional args, if provided.
    let spawned_id = spawn_initial_command(&cli, &manager, &cfg).await?;

    if cfg.display.enabled {
        // ── Display mode (local terminal VTTY) ──
        // Create a shutdown channel for the display loop.
        let (shutdown_tx, _shutdown_rx) = broadcast::channel::<()>(1);

        // Install signal handler for clean shutdown in no-server mode.
        // Without the server's signal handler, we need our own.
        spawn_no_server_signal_handler(shutdown_tx.clone());

        let log_entries = manager.logger().memory_buffer_arc();
        let effective_display_all = cfg.display.display_all || cfg.interactive.tabs;
        run_display_loop(
            &manager,
            spawned_id.as_deref(),
            cfg.display.refresh_ms,
            effective_display_all,
            shutdown_tx,
            &cfg.interactive.keybindings,
            &log_entries,
            cfg.interactive.tabs,
        )
        .await;
    } else if let Some(ref id) = spawned_id {
        // ── Headless mode: wait for the child to exit ──
        wait_for_child(&manager, id).await;
    } else {
        // No command and no display — nothing to do
        tracing::info!("No command specified and no display. Exiting.");
    }

    Ok(())
}

/// Signal handler for --no-server mode.
/// Sends on the shutdown channel when SIGINT/SIGTERM is received.
fn spawn_no_server_signal_handler(shutdown_tx: broadcast::Sender<()>) {
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
                    tracing::info!("Received SIGINT, exiting");
                    let _ = shutdown_tx.send(());
                }
                _ = sigterm.recv() => {
                    tracing::info!("Received SIGTERM, exiting");
                    let _ = shutdown_tx.send(());
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

fn main() -> Result<()> {
    // Phase 1: Synchronous pre-runtime (no tokio threads yet)
    let cli = match dispatch::pre_runtime()? {
        Some(cli) => cli,
        None => return Ok(()), // Subcommand handled, exit
    };

    // Daemonize if requested — MUST happen before tokio::runtime is created.
    if cli.daemon {
        #[cfg(unix)]
        {
            let mut cfg = dispatch::resolve_config(&cli)?;

            if !cfg.daemon.enabled {
                cfg.daemon.enabled = true;
            }

            vrunner::daemon::unix::daemonize(&cfg)?;
        }
        #[cfg(not(unix))]
        {
            anyhow::bail!("--daemon is only supported on Unix-like systems");
        }
    }

    // Optimization #9: Use current_thread runtime when --no-server is set.
    // The current_thread runtime avoids spawning worker threads and I/O
    // driver threads, saving ~5-10ms of startup overhead.  A multi-thread
    // runtime is only needed when the web server is running (to handle
    // concurrent HTTP connections).
    if cli.no_server {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?
            .block_on(async_main(cli))
    } else {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?
            .block_on(async_main(cli))
    }
}
