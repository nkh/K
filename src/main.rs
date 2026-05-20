use anyhow::Result;
use clap::Parser;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};

use vrunner::cli::args::{Cli, Commands, CertAction};
use vrunner::config::loader::load_config;
use vrunner::config::merge::apply_profile;
use vrunner::daemon;
use vrunner::instance::registry::InstanceRegistry;
use vrunner::process::manager::CommandManager;
use vrunner::web::auth::AuthManager;
use vrunner::web::certs::CertificateStore;
use vrunner::web::server::start_server;

/// Synchronous pre-runtime phase: parse CLI, handle subcommands, load config,
/// and daemonize. Daemonization MUST happen before the tokio runtime starts,
/// because fork() only copies the calling thread while tokio's multi-threaded
/// runtime creates internal threads for I/O, timers, and blocking tasks.
fn pre_runtime() -> Result<Option<Cli>> {
    let cli = Cli::parse();

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
        Some(Commands::Freeze { .. }) => {
            // freeze is async (uses reqwest), fall through to async phase
        }
        Some(Commands::Thaw { .. }) => {
            // thaw is async (uses reqwest), fall through to async phase
        }
        Some(Commands::Cert { action }) => {
            // Cert subcommands are synchronous — handle them here
            handle_cert_command(action)?;
            return Ok(None);
        }
        None => {}
    }

    Ok(Some(cli))
}

/// Async runtime phase: start the server and manage the application lifecycle.
async fn async_main(cli: Cli) -> Result<()> {
    // Initialize tracing (after daemonize, so logs go to the right place)
    tracing_subscriber::fmt::init();

    // Handle list subcommand — query running instances and show their commands
    if let Some(Commands::List) = cli.command {
        handle_list_command(&cli).await?;
        return Ok(());
    }

    // Handle stop subcommand (needs async for HTTP request)
    if let Some(Commands::Stop { pid }) = cli.command {
        // First try to stop a specific command by PID on any instance
        let stopped = handle_stop_command_by_pid(&cli, pid).await?;
        if !stopped {
            // Fall back to stopping the whole instance
            let registry = InstanceRegistry::new()?;
            registry.stop_instance(pid).await?;
        }
        return Ok(());
    }

    // Handle spawn subcommand — send to a running vrunner instance
    if let Some(Commands::Spawn { ref cmd, ref args }) = cli.command {
        handle_spawn_command(&cli, &cmd, &args).await?;
        return Ok(());
    }

    // Handle freeze subcommand
    if let Some(Commands::Freeze { ref id }) = cli.command {
        handle_freeze_command(&cli, &id).await?;
        return Ok(());
    }

    // Handle thaw subcommand
    if let Some(Commands::Thaw { ref id }) = cli.command {
        handle_thaw_command(&cli, &id).await?;
        return Ok(());
    }

    // Load and merge configuration
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
                    cfg.profiles.entries.keys().cloned().collect::<Vec<_>>().join(", ")
                }
            );
        }
    }

    // Apply CLI overrides (highest precedence)
    cli.apply_overrides(&mut cfg);

    // When --display is enabled, detect the real terminal size and use it
    // for the VTTY so that the child process (e.g. htop) formats its output
    // for the actual visible area.  However, if the user explicitly set
    // --vtty-rows or --vtty-cols on the command line, those take precedence.
    //
    // We use a robust multi-method detection:
    //   1. ioctl(TIOCGWINSZ) on /dev/tty (most reliable on Unix)
    //   2. ioctl(TIOCGWINSZ) on stdout (crossterm's approach)
    //   3. COLUMNS/LINES env vars as last resort
    if cfg.display.enabled {
        let detected = detect_terminal_size();
        if let Some((rows, cols)) = detected {
            tracing::info!(rows, cols, method = "multi", "Detected terminal size for display mode");
            if cli.vtty_rows.is_none() {
                cfg.vtty.rows = rows;
            }
            if cli.vtty_cols.is_none() {
                cfg.vtty.cols = cols;
            }
        } else {
            tracing::warn!("Failed to detect terminal size, using config defaults");
        }
    }

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

    // If a child command was provided, spawn it immediately
    let spawned_id = if let Some(cmd_args) = cli.cmd_args {
        if !cmd_args.is_empty() {
            let cmd = cmd_args[0].clone();
            let args = cmd_args[1..].to_vec();
            let id = manager.spawn(cmd, args, None, cfg.environment.variables.clone()).await?;
            Some(id)
        } else {
            None
        }
    } else {
        None
    };

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
            ).await
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
        run_display_loop(
            &manager,
            spawned_id.as_deref(),
            cfg.display.refresh_ms,
            shutdown_tx.clone(),
        ).await;
    } else if let Some(ref id) = spawned_id {
        // ── Headless mode with direct child ──
        // No display, but a child was spawned directly via CLI.  Wait for
        // the child process to exit, then shut down the server.
        wait_for_child(&manager, id).await;
        let _ = shutdown_tx.send(());
    } else {
        // ── Idle server mode ──
        // No display, no direct child.  Wait for an external shutdown
        // signal (SIGINT, SIGTERM, or the /api/shutdown endpoint).
        let mut rx = shutdown_tx.subscribe();
        let _ = rx.recv().await;
    }

    // Wait for server to finish — propagate both JoinError and server errors
    server_handle.await??;

    // Cleanup on exit
    registry.unregister_current()?;

    Ok(())
}

/// Run the interactive terminal display loop.
///
/// This function blocks (in the async sense) until the user quits (Ctrl+\),
/// the direct child exits, all commands have exited, or a shutdown signal
/// is received.  It renders the VTTY buffer to the local terminal using
/// crossterm, forwards all keystrokes to the active child command, and
/// handles SIGWINCH by resizing both the PTY master and the VTTY buffer.
async fn run_display_loop(
    manager: &Arc<CommandManager>,
    direct_child_id: Option<&str>,
    refresh_ms: u64,
    shutdown_tx: broadcast::Sender<()>,
) {
    use vrunner::vtty::display::TerminalDisplay;
    use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
    use crossterm::{cursor, ExecutableCommand};

    // Set up the alternate screen and raw mode.
    let mut stdout = std::io::stdout();
    if let Err(e) = terminal::enable_raw_mode() {
        tracing::warn!(error = %e, "Failed to enable raw mode");
        return;
    }
    let _ = stdout.execute(EnterAlternateScreen);
    let _ = stdout.execute(cursor::Hide);

    // Spawn a blocking stdin reader that sends individual bytes through a
    // channel.  In raw mode each keystroke is available immediately.
    let (stdin_tx, mut stdin_rx) = mpsc::channel::<u8>(128);
    tokio::task::spawn_blocking(move || {
        use std::io::Read;
        let mut stdin = std::io::stdin();
        let mut buf = [0u8; 1];
        loop {
            match stdin.read(&mut buf) {
                Ok(1) => {
                    if stdin_tx.blocking_send(buf[0]).is_err() {
                        break; // receiver dropped
                    }
                }
                Ok(_) => break, // EOF or unexpected read size
                Err(_) => break, // read error
            }
        }
    });

    // Set up SIGWINCH handler for terminal resize.
    // When the user resizes their terminal emulator, the kernel delivers
    // SIGWINCH to the foreground process group.  We catch it here and
    // propagate the new size to both the PTY master (so the child gets
    // its own SIGWINCH) and the in-memory VTTY buffer.
    let mut winch_rx = {
        use tokio::signal::unix::{signal, SignalKind};
        match signal(SignalKind::window_change()) {
            Ok(stream) => stream,
            Err(e) => {
                tracing::warn!(error = %e, "Failed to create SIGWINCH handler");
                // Create a channel that never fires as fallback
                let (_tx, rx) = tokio::sync::mpsc::channel::<()>(1);
                // Convert to a stream-like by wrapping in a manual poll
                // Actually, just return and skip WINCH handling entirely
                drop(rx);
                return;
            }
        }
    };

    let mut shutdown_rx = shutdown_tx.subscribe();
    let interval = tokio::time::Duration::from_millis(refresh_ms);
    // Track which command we're displaying so we can forward keystrokes.
    let mut active_id: Option<String> = direct_child_id.map(String::from);

    loop {
        tokio::select! {
            // ── Periodic VTTY render ──
            _ = tokio::time::sleep(interval) => {
                // Find the command to display (prefer the direct child)
                let commands = manager.list();
                let target_id = active_id.as_ref()
                    .or_else(|| commands.first().map(|(id, _, _, _, _)| id))
                    .cloned();

                let mut should_break = false;

                if let Some(ref id) = target_id {
                    if let Some(handle) = manager.get(id) {
                        // Check if the child process is still alive.
                        // NOTE: is_alive() uses kill(pid, 0) which returns 0
                        // even for zombie processes.  The zombie is reaped by
                        // child.wait() in the spawner, after which the PID no
                        // longer exists and kill returns -1.
                        let alive = handle.is_alive();
                        if !alive {
                            tracing::info!(id = %id, pid = handle.pid, "Active command exited, shutting down");
                            should_break = true;
                        } else {
                            // Render the VTTY buffer to the terminal.
                            let buf = handle.vtty_snapshot().await;
                            drop(handle);
                            let _ = TerminalDisplay::render(&buf);
                        }
                    } else {
                        // Command was removed (killed via API).
                        tracing::info!("Active command removed from manager");
                        should_break = true;
                    }
                } else {
                    // No commands at all — if this is a direct-child session,
                    // exit.  If commands were spawned via API, keep running
                    // (the user might spawn more).
                    if direct_child_id.is_some() {
                        tracing::info!("All commands exited, shutting down");
                        should_break = true;
                    } else {
                        let _ = TerminalDisplay::clear();
                    }
                }

                if should_break {
                    let _ = TerminalDisplay::clear();
                    break;
                }

                active_id = target_id;
            }

            // ── SIGWINCH — terminal resize ──
            _ = winch_rx.recv() => {
                // Detect the new terminal size using the same multi-method
                // approach as the initial size detection.
                if let Some((rows, cols)) = detect_terminal_size() {
                    tracing::debug!(rows, cols, "SIGWINCH: terminal resized");

                    // Resize all active commands' VTTY + PTY.
                    // This ensures that:
                    //   1. The PTY master is resized → kernel sends SIGWINCH
                    //      to the child process (e.g. htop, vim)
                    //   2. The in-memory VTTY buffer matches the new size
                    for entry in manager.list() {
                        let id = &entry.0;
                        if let Some(handle) = manager.get(id) {
                            if let Err(e) = handle.resize_pty(rows, cols).await {
                                tracing::warn!(
                                    id = %id, rows, cols, error = %e,
                                    "Failed to resize command on WINCH"
                                );
                            }
                        }
                    }
                }
            }

            // ── Keystroke forwarding ──
            byte = stdin_rx.recv() => {
                match byte {
                    Some(b) if b == 0x1c => {
                        // Ctrl+\ (SIGQUIT) — quit display mode
                        break;
                    }
                    Some(b) => {
                        // Forward the byte to the active command.
                        if let Some(ref id) = active_id {
                            if let Some(handle) = manager.get(id) {
                                let _ = handle.send_bytes(vec![b]).await;
                            }
                        }
                    }
                    None => {
                        // Stdin channel closed (EOF).
                        break;
                    }
                }
            }

            // ── External shutdown ──
            _ = shutdown_rx.recv() => {
                break;
            }
        }
    }

    // Restore the terminal before returning.
    let _ = stdout.execute(cursor::Show);
    let _ = stdout.execute(LeaveAlternateScreen);
    let _ = terminal::disable_raw_mode();

    // Trigger server shutdown so async_main can unwind.
    let _ = shutdown_tx.send(());
}

/// Wait for a direct child command to exit (headless, non-display mode).
///
/// Polls `kill(pid, 0)` at 500 ms intervals.  When the child is no longer
/// alive the function returns.
async fn wait_for_child(manager: &Arc<CommandManager>, id: &str) {
    loop {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        if let Some(handle) = manager.get(&id.to_string()) {
            let pid = handle.pid as i32;
            let alive = unsafe { libc::kill(pid, 0) == 0 };
            if !alive {
                tracing::info!(id, "Direct child exited");
                return;
            }
        } else {
            tracing::info!(id, "Direct child removed from manager");
            return;
        }
    }
}

/// Detect the terminal size using multiple methods, returning the most
/// reliable result.  Tries /dev/tty first (always the controlling terminal),
/// then stdout, then COLUMNS/LINES environment variables.
#[cfg(unix)]
fn detect_terminal_size() -> Option<(u16, u16)> {
    use std::fs::File;
    use std::os::fd::AsRawFd;

    // Method 1: ioctl(TIOCGWINSZ) on /dev/tty — the controlling terminal.
    // This is the most reliable method because /dev/tty always refers to the
    // controlling terminal even if stdout has been redirected.
    if let Ok(tty) = File::open("/dev/tty") {
        let fd = tty.as_raw_fd();
        let mut size: libc::winsize = unsafe { std::mem::zeroed() };
        if unsafe { libc::ioctl(fd, libc::TIOCGWINSZ, &mut size) } == 0 {
            let rows = size.ws_row;
            let cols = size.ws_col;
            if rows > 0 && cols > 0 {
                tracing::debug!(
                    rows, cols, method = "/dev/tty",
                    "Terminal size from /dev/tty"
                );
                return Some((rows, cols));
            }
        }
    }

    // Method 2: crossterm on stdout (uses ioctl on stdout fd).
    // This works when stdout is directly connected to a terminal.
    // NOTE: crossterm::terminal::size() returns (columns, rows), NOT (rows, columns).
    if let Ok((cols, rows)) = crossterm::terminal::size() {
        if rows > 0 && cols > 0 {
            tracing::debug!(
                rows, cols, method = "crossterm",
                "Terminal size from crossterm (stdout)"
            );
            return Some((rows, cols));
        }
    }

    // Method 3: COLUMNS / LINES environment variables.
    if let (Ok(cols_str), Ok(rows_str)) = (
        std::env::var("COLUMNS"),
        std::env::var("LINES"),
    ) {
        if let (Ok(cols), Ok(rows)) = (cols_str.parse::<u16>(), rows_str.parse::<u16>()) {
            if rows > 0 && cols > 0 {
                tracing::debug!(
                    rows, cols, method = "env",
                    "Terminal size from COLUMNS/LINES env vars"
                );
                return Some((rows, cols));
            }
        }
    }

    tracing::warn!("All terminal size detection methods failed");
    None
}

#[cfg(not(unix))]
fn detect_terminal_size() -> Option<(u16, u16)> {
    // crossterm returns (columns, rows); we need (rows, columns).
    crossterm::terminal::size().ok().map(|(cols, rows)| (rows, cols))
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
            let cfg = load_config(cli.config.as_deref())?;
            let mut cfg = cfg;

            // Apply profile if specified
            if let Some(ref profile_name) = cli.profile {
                if let Some(profile) = cfg.profiles.entries.clone().get(profile_name) {
                    cfg = apply_profile(cfg, profile);
                }
            }

            cli.apply_overrides(&mut cfg);

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

/// Build the base URL for a vrunner instance, handling auth and TLS.
fn instance_url(info: &vrunner::instance::info::InstanceInfo, _auth_token: &Option<String>) -> String {
    let scheme = if info.port == 443 { "https" } else { "http" };
    let mut url = format!("{}://{}:{}", scheme, info.bind, info.port);
    // For simplicity, we try HTTP first. TLS instances will reject and
    // the error message will guide the user.
    url = format!("http://{}:{}", info.bind, info.port);
    url
}

/// Discover running vrunner instances and resolve to a single target.
/// Returns the selected InstanceInfo or an error.
fn resolve_instance(
    cli: &Cli,
    registry: &InstanceRegistry,
) -> Result<vrunner::instance::info::InstanceInfo> {
    let instances = registry.list_instances();

    if instances.is_empty() {
        anyhow::bail!("No running vrunner instances found. Start one first with: vrunner -- <command>");
    }

    // If --target PID was specified, use that instance
    if let Some(target_pid) = cli.target {
        match instances.iter().find(|i| i.pid == target_pid) {
            Some(info) => return Ok(info.clone()),
            None => anyhow::bail!(
                "No vrunner instance found with PID {}. Running instances:\n{}",
                target_pid,
                format_instance_list(&instances)
            ),
        }
    }

    // Only one instance — use it automatically
    if instances.len() == 1 {
        return Ok(instances.into_iter().next().unwrap());
    }

    // Multiple instances — prompt the user
    eprintln!("Multiple vrunner instances are running:");
    eprintln!("{}", format_instance_list(&instances));
    eprintln!();
    eprint!("Enter the PID of the instance to use (or Ctrl+C to abort): ");
    eprintln!();

    // Since we can't easily read stdin in all contexts (piped, daemon, etc.),
    // return an error with instructions
    anyhow::bail!(
        "Multiple vrunner instances are running. Use --target PID to select one.\n\
         Running instances:\n{}",
        format_instance_list(&instances)
    );
}

fn format_instance_list(instances: &[vrunner::instance::info::InstanceInfo]) -> String {
    let mut out = String::new();
    out.push_str(&format!("{:<10} {:<8} {:<20} {:<10} {:<10} COMMAND\n",
        "PID", "PORT", "BIND", "DAEMON", "DISPLAY"));
    for info in instances {
        out.push_str(&format!("{:<10} {:<8} {:<20} {:<10} {:<10} {}\n",
            info.pid,
            info.port,
            info.bind,
            if info.daemon { "yes" } else { "no" },
            if info.display { "yes" } else { "no" },
            info.command.as_deref().unwrap_or("(idle)")
        ));
    }
    out
}

/// Handle the `vrunner spawn` subcommand.
/// Discovers a running vrunner instance and sends a spawn request via HTTP API.
async fn handle_spawn_command(cli: &Cli, cmd: &str, args: &[String]) -> Result<()> {
    let registry = InstanceRegistry::new()?;
    let info = resolve_instance(cli, &registry)?;

    let url = instance_url(&info, &None);
    let client = reqwest::Client::new();

    let mut body = serde_json::json!({
        "cmd": cmd,
        "args": args,
    });

    // Add --env variables if provided
    let cli_env = cli.parse_env_vars();
    if !cli_env.is_empty() {
        body["env"] = serde_json::json!(cli_env);
    }

    // Add --no-env flag to skip config-level environment
    if cli.no_env {
        body["no_env"] = serde_json::json!(true);
    }

    // Add exit configuration if provided
    if let Some(ref on_exit) = cli.on_exit {
        body["on_exit"] = serde_json::json!(on_exit);
    }
    if let Some(ref on_error) = cli.on_error {
        body["on_error"] = serde_json::json!(on_error);
    }
    if let Some(timeout) = cli.exit_timeout {
        body["exit_timeout"] = serde_json::json!(timeout);
    }

    // Add profile if specified
    if let Some(ref profile) = cli.profile {
        body["profile"] = serde_json::json!(profile);
    }

    tracing::info!(target_pid = info.pid, cmd = cmd, "Spawning command on remote instance");

    let resp = client
        .post(format!("{}/api/commands", url))
        .json(&body)
        .send()
        .await?;

    let status = resp.status();
    let result: serde_json::Value = resp.json().await?;

    if status.is_success() {
        let id = result["data"]["id"].as_str().unwrap_or("?");
        println!("Command spawned successfully on instance {} (PID {})", info.pid, info.pid);
        println!("  Command ID: {}", id);
        println!("  VTTY:      {}/api/commands/{}/vtty/html", url, id);
    } else {
        let error = result["error"].as_str().unwrap_or("Unknown error");
        eprintln!("Failed to spawn command: {}", error);
        std::process::exit(1);
    }

    Ok(())
}

/// Handle the `vrunner freeze` subcommand.
async fn handle_freeze_command(cli: &Cli, id: &str) -> Result<()> {
    let registry = InstanceRegistry::new()?;
    let info = resolve_instance(cli, &registry)?;
    let url = instance_url(&info, &None);
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{}/api/commands/{}/freeze", url, id))
        .send()
        .await?;

    let status = resp.status();
    let result: serde_json::Value = resp.json().await?;

    if status.is_success() {
        println!("Command {} frozen (SIGSTOP) on instance {}", id, info.pid);
    } else {
        let error = result["error"].as_str().unwrap_or("Unknown error");
        eprintln!("Failed to freeze command: {}", error);
        std::process::exit(1);
    }

    Ok(())
}

/// Handle the `vrunner thaw` subcommand.
async fn handle_thaw_command(cli: &Cli, id: &str) -> Result<()> {
    let registry = InstanceRegistry::new()?;
    let info = resolve_instance(cli, &registry)?;
    let url = instance_url(&info, &None);
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{}/api/commands/{}/thaw", url, id))
        .send()
        .await?;

    let status = resp.status();
    let result: serde_json::Value = resp.json().await?;

    if status.is_success() {
        println!("Command {} thawed (SIGCONT) on instance {}", id, info.pid);
    } else {
        let error = result["error"].as_str().unwrap_or("Unknown error");
        eprintln!("Failed to thaw command: {}", error);
        std::process::exit(1);
    }

    Ok(())
}

/// Handle the `vrunner list` subcommand.
/// Queries all running instances and shows their commands with arguments.
async fn handle_list_command(_cli: &Cli) -> Result<()> {
    let registry = InstanceRegistry::new()?;
    let instances = registry.list_instances();

    if instances.is_empty() {
        println!("No running vrunner instances.");
        return Ok(());
    }

    let client = reqwest::Client::new();

    println!("{:<10} {:<8} {:<20} {:<10} {:<10} COMMAND",
        "PID", "PORT", "BIND", "DAEMON", "DISPLAY");

    for info in &instances {
        let url = instance_url(info, &None);
        let label = if info.display { "yes" } else { "no" };
        let daemon = if info.daemon { "yes" } else { "no" };

        // Query the instance's command list
        let cmd_str = info.command.as_deref().unwrap_or("(idle)");
        let mut printed_instance = false;

        // Try to fetch commands from the instance API
        match client.get(format!("{}/api/commands", url)).send().await {
            Ok(resp) => {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    if json["status"] == "ok" {
                        if let Some(cmds) = json["data"].as_array() {
                            if cmds.is_empty() {
                                println!("{:<10} {:<8} {:<20} {:<10} {:<10} {} (no commands)",
                                    info.pid, info.port, info.bind, daemon, label, cmd_str);
                            } else {
                                for (i, cmd) in cmds.iter().enumerate() {
                                    let name = cmd["name"].as_str().unwrap_or("?");
                                    let args = cmd["args"].as_array()
                                        .map(|a| a.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
                                        .unwrap_or_default();
                                    let args_str = if args.is_empty() {
                                        String::new()
                                    } else {
                                        format!(" {:?}", args)
                                    };
                                    let _pid = cmd["pid"].as_u64().unwrap_or(0);
                                    let _id_short = cmd["id"].as_str()
                                        .map(|id| &id[..8])
                                        .unwrap_or("?");
                                    let cert = cmd["certificate"].as_str();

                                    let line = if i == 0 {
                                        format!("{:<10} {:<8} {:<20} {:<10} {:<10} {} -> {}{} [{}]",
                                            info.pid, info.port, info.bind, daemon, label,
                                            cmd_str, name, args_str,
                                            cert.unwrap_or("-"))
                                    } else {
                                        format!("{:<10} {:<8} {:<20} {:<10} {:<10}     {}{}{} [{}]",
                                            "", "", "", "", "",
                                            cmd_str, name, args_str,
                                            cert.unwrap_or("-"))
                                    };
                                    println!("{}", line);
                                }
                            }
                            printed_instance = true;
                        }
                    }
                }
            }
            Err(e) => {
                // Instance not reachable — show basic info
                println!("{:<10} {:<8} {:<20} {:<10} {:<10} {} (unreachable: {})",
                    info.pid, info.port, info.bind, daemon, label, cmd_str, e);
                printed_instance = true;
            }
        }

        if !printed_instance {
            println!("{:<10} {:<8} {:<20} {:<10} {:<10} {}",
                info.pid, info.port, info.bind, daemon, label, cmd_str);
        }
    }

    Ok(())
}

/// Try to stop a specific command by PID on any running instance.
/// Returns true if the command was found and stopped, false otherwise.
async fn handle_stop_command_by_pid(_cli: &Cli, pid: u32) -> Result<bool> {
    let registry = InstanceRegistry::new()?;
    let instances = registry.list_instances();

    if instances.is_empty() {
        return Ok(false);
    }

    let client = reqwest::Client::new();

    for info in &instances {
        let url = instance_url(info, &None);
        let resp = client
            .post(format!("{}/api/commands/kill-pid/{}", url, pid))
            .send()
            .await;

        if let Ok(resp) = resp {
            if resp.status().is_success() {
                println!("Command with PID {} stopped on instance {} (PID {})", pid, info.pid, info.pid);
                return Ok(true);
            }
        }
    }

    Ok(false)
}

/// Handle the `vrunner cert` subcommands (generate, list, show, remove).
///
/// These are synchronous operations that don't require the tokio runtime.
fn handle_cert_command(action: &CertAction) -> Result<()> {
    match action {
        CertAction::Generate { name } => {
            let mut store = CertificateStore::new();
            let entry = store.generate(name)?;
            let token = entry.derive_token()?;
            println!("Certificate '{}' generated successfully.", name);
            println!("  Certificate: {}", entry.cert_file);
            println!("  Key:        {}", entry.key_file);
            println!("  Token:      {}... (first 16 of 64 chars)", &token[..16]);
        }
        CertAction::List => {
            let cfg = load_config(None)?;
            let entries: Vec<vrunner::web::certs::CertificateEntry> = cfg
                .certificates
                .entries
                .iter()
                .map(|e| vrunner::web::certs::CertificateEntry {
                    name: e.name.clone(),
                    cert_file: e.cert_file.clone(),
                    key_file: e.key_file.clone(),
                })
                .collect();

            if entries.is_empty() {
                println!("No certificates configured.");
                return Ok(());
            }

            match CertificateStore::load_or_generate(entries) {
                Ok(store) => {
                    let certs = store.list();
                    if certs.is_empty() {
                        println!("No certificates in the store.");
                    } else {
                        println!("{:<25} {:<50} {}", "NAME", "CERT FILE", "TOKEN (prefix)");
                        println!("{}", "-".repeat(100));
                        for cert in certs {
                            let token_preview = cert
                                .derive_token()
                                .map(|t| format!("{}...", &t[..16]))
                                .unwrap_or_else(|_| "<error>".to_string());
                            println!("{:<25} {:<50} {}", cert.name, cert.cert_file, token_preview);
                        }
                    }
                }
                Err(e) => {
                    anyhow::bail!("Failed to load certificates: {}", e);
                }
            }
        }
        CertAction::Show { name } => {
            let cfg = load_config(None)?;
            let entries: Vec<vrunner::web::certs::CertificateEntry> = cfg
                .certificates
                .entries
                .iter()
                .map(|e| vrunner::web::certs::CertificateEntry {
                    name: e.name.clone(),
                    cert_file: e.cert_file.clone(),
                    key_file: e.key_file.clone(),
                })
                .collect();

            let store = CertificateStore::load_or_generate(entries)?;

            match store.get(name) {
                Some(entry) => {
                    let token = entry.derive_token()?;
                    println!("Certificate: {}", entry.name);
                    println!("  Certificate: {}", entry.cert_file);
                    println!("  Key:        {}", entry.key_file);
                    println!("  Token:      {} (full SHA-256 hex)", token);
                    println!("  Token (16): {}...", &token[..16]);
                }
                None => {
                    anyhow::bail!("Certificate '{}' not found in store", name);
                }
            }
        }
        CertAction::Remove { name } => {
            let cfg = load_config(None)?;
            let entries: Vec<vrunner::web::certs::CertificateEntry> = cfg
                .certificates
                .entries
                .iter()
                .map(|e| vrunner::web::certs::CertificateEntry {
                    name: e.name.clone(),
                    cert_file: e.cert_file.clone(),
                    key_file: e.key_file.clone(),
                })
                .collect();

            let mut store = CertificateStore::load_or_generate(entries)?;

            match store.remove(name) {
                Some(entry) => {
                    println!("Certificate '{}' removed from store.", name);
                    println!("  Certificate: {}", entry.cert_file);
                    println!("  Key:        {}", entry.key_file);
                    println!("  Note: Files were not deleted.");
                }
                None => {
                    anyhow::bail!("Certificate '{}' not found in store", name);
                }
            }
        }
    }
    Ok(())
}
