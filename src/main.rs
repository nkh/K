use anyhow::Result;
use clap::Parser;
use crossterm::style::Color;
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

/// Colorize text using crossterm when stdout is a TTY, plain text otherwise.
fn c(text: &str, color: Color, bold: bool) -> String {
    use crossterm::style::Stylize;
    use std::io::IsTerminal;
    if !std::io::stdout().is_terminal() {
        return text.to_string();
    }
    let styled = text.with(color);
    if bold { styled.bold().to_string() } else { styled.to_string() }
}

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
        Some(Commands::Freeze { pid: _ }) => {
            // freeze is async (uses reqwest), fall through to async phase
        }
        Some(Commands::Thaw { pid: _ }) => {
            // thaw is async (uses reqwest), fall through to async phase
        }
        Some(Commands::Cert { action }) => {
            // Cert subcommands are synchronous — handle them here
            handle_cert_command(action)?;
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
        Some(Commands::Resize { .. }) => {
            // resize-command is async (needs HTTP), fall through to async phase
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
        let registry = InstanceRegistry::new()?;
        let instances = registry.list_instances();
        let client = reqwest::Client::new();
        let stopped = handle_stop_command_by_pid_on_instances(&client, &instances, pid).await?;
        if !stopped {
            // Fall back to stopping the whole instance
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
    if let Some(Commands::Freeze { pid }) = cli.command {
        handle_freeze_command(&cli, pid).await?;
        return Ok(());
    }

    // Handle thaw subcommand
    if let Some(Commands::Thaw { pid }) = cli.command {
        handle_thaw_command(&cli, pid).await?;
        return Ok(());
    }

    // Handle list-vrunner subcommand
    if let Some(Commands::ListVrunner) = cli.command {
        handle_list_vrunner_command(&cli).await?;
        return Ok(());
    }

    // Handle list-commands subcommand
    if let Some(Commands::ListCommands) = cli.command {
        handle_list_commands_command(&cli).await?;
        return Ok(());
    }

    // Handle stop-command subcommand
    if let Some(Commands::StopCommand { ref target }) = cli.command {
        let stopped = handle_stop_command(&cli, target).await?;
        if !stopped {
            eprintln!("No matching command found for '{}'. Use `vrunner list` to see running commands.", target);
            std::process::exit(1);
        }
        return Ok(());
    }

    // Handle resize-command subcommand
    if let Some(Commands::Resize { ref target, rows, cols }) = cli.command {
        handle_resize_command(&cli, target, rows, cols).await?;
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
            cfg.display.display_all,
            shutdown_tx.clone(),
        ).await;
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

/// Run the interactive terminal display loop.
///
/// This function blocks (in the async sense) until the user quits (Ctrl+\),
/// all commands have exited, a shutdown signal is received, or (when
/// `display_all` is false) the direct CLI command exits while other commands
/// remain.
///
/// ## Display modes after direct child exits
///
/// - `display_all == true`: the loop transitions to "monitor mode" —
///   `active_id` is cleared, and the display falls back to the first available
///   command.  Keystrokes are forwarded to that command.  The loop only breaks
///   (and sends shutdown) when the command manager is empty.
///
/// - `display_all == false` (default): the display is dismissed — the
///   alternate screen is torn down and a status message is printed.  The
///   function returns *without* sending shutdown, allowing the caller to
///   idle-wait for the remaining commands or an explicit shutdown signal.
///
/// It renders the VTTY buffer to the local terminal using crossterm,
/// forwards all keystrokes to the active child command, and handles
/// SIGWINCH by resizing both the PTY master and the VTTY buffer.
///
/// Exit detection: when a direct child was spawned, the loop monitors two
/// signals:
///   1. `is_alive()` — uses kill(pid, 0), returns false after the child is
///      reaped by the spawner's child.wait().
///   2. Manager removal — if the command is removed from the DashMap (e.g.
///      via kill API), we break immediately.
///
/// Additionally, the spawner now removes the command from the manager after
/// the child exits, so the manager.get() check is the most reliable signal.
async fn run_display_loop(
    manager: &Arc<CommandManager>,
    direct_child_id: Option<&str>,
    refresh_ms: u64,
    display_all: bool,
    shutdown_tx: broadcast::Sender<()>,
) -> bool {
    use vrunner::vtty::display::TerminalDisplay;
    use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
    use crossterm::{cursor, ExecutableCommand};
    use std::io::Write;

    // Set up the alternate screen and raw mode.
    let mut stdout = std::io::stdout();
    if let Err(e) = terminal::enable_raw_mode() {
        tracing::warn!(error = %e, "Failed to enable raw mode");
        return true;  // fatal setup error — shut down
    }
    let _ = stdout.execute(EnterAlternateScreen);
    let _ = stdout.execute(cursor::Hide);

    // ── Keystroke reading via /dev/tty + AsyncFd ──
    //
    // We read from /dev/tty (not tokio::io::stdin()) because tokio's Stdin
    // wraps a synchronous read in a spawn_blocking thread.  When the display
    // loop breaks on child exit, the future is dropped but the blocking
    // thread stays stuck on read().  During Runtime::drop, tokio waits for
    // all blocking threads to complete → the process hangs until the user
    // presses Enter.
    //
    // AsyncFd registers the fd with the tokio reactor.  When the future is
    // dropped (select! picks another branch), the registration is removed
    // — no thread, no hang.  We explicitly set O_NONBLOCK so that read()
    // never blocks even if the kernel delivers a spurious readiness event.
    use std::os::fd::AsRawFd;
    let tty_async: tokio::io::unix::AsyncFd<std::fs::File> = {
        let tty = match std::fs::File::open("/dev/tty") {
            Ok(f) => f,
            Err(e) => {
                tracing::error!(error = %e, "Failed to open /dev/tty");
                let _ = terminal::disable_raw_mode();
                let _ = stdout.execute(cursor::Show);
                let _ = stdout.execute(LeaveAlternateScreen);
                return true;  // fatal — shut down
            }
        };
        // Ensure non-blocking mode — AsyncFd relies on EAGAIN/EWOULDBLOCK
        // to detect "no more data".  On Linux, opening /dev/tty inherits
        // the terminal's blocking mode, so we must set O_NONBLOCK explicitly.
        unsafe {
            let flags = libc::fcntl(tty.as_raw_fd(), libc::F_GETFL);
            libc::fcntl(tty.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK);
        }
        match tokio::io::unix::AsyncFd::new(tty) {
            Ok(a) => a,
            Err(e) => {
                tracing::error!(error = %e, "Failed to create AsyncFd for /dev/tty");
                let _ = terminal::disable_raw_mode();
                let _ = stdout.execute(cursor::Show);
                let _ = stdout.execute(LeaveAlternateScreen);
                return true;  // fatal — shut down
            }
        }
    };
    let mut stdin_buf = [0u8; 1];

    // Set up SIGWINCH handler for terminal resize.
    // When the user resizes their terminal emulator, the kernel delivers
    // SIGWINCH to the foreground process group.  We catch it here and
    // propagate the new size to both the PTY master (so the child gets
    // its own SIGWINCH) and the in-memory VTTY buffer.
    //
    // SIGWINCH handling is optional — if signal() fails we bridge through
    // an mpsc channel that never fires so the select! branch is simply
    // never taken.  We must NOT return here because raw mode is already
    // enabled and the alternate screen is active — returning without
    // cleanup would leave the terminal in a broken state.
    let mut winch_rx = {
        use tokio::signal::unix::{signal, SignalKind};
        let (winch_tx, winch_bridge) = tokio::sync::mpsc::channel::<()>(4);
        match signal(SignalKind::window_change()) {
            Ok(mut stream) => {
                // Spawn a forwarding task so the select! branch type
                // is always mpsc::Receiver, never SignalStream.
                tokio::spawn(async move {
                    while stream.recv().await.is_some() {
                        if winch_tx.send(()).await.is_err() {
                            break;
                        }
                    }
                });
            }
            Err(e) => {
                tracing::warn!(error = %e, "SIGWINCH unavailable — terminal resize not propagated");
                // winch_bridge never fires — the select! branch is inert.
            }
        }
        winch_bridge
    };

    let mut shutdown_rx = shutdown_tx.subscribe();
    // Track which command we're displaying so we can forward keystrokes.
    // Must be `mut` so we can clear it when transitioning to monitor mode
    // (when the direct child exits but other commands remain).
    let mut active_id: Option<String> = direct_child_id.map(String::from);

    // When `display_all == false` and the direct child exits, the display
    // enters "dismissed" state: the alternate screen stays active, a status
    // message is rendered, and the loop waits for 'q' / Ctrl+C to shut down.
    // We stay in the same raw-mode / AsyncFd loop that was already working —
    // no need to tear down and re-create the terminal setup.
    let mut dismissed = false;
    let mut dismiss_rendered = false;

    // ── Event-driven exit detection ──
    // We use a tokio::sync::watch channel instead of Notify.
    // Notify loses notifications when no waiter is present — if the child
    // exits during the 100ms server startup delay, the notification is
    // permanently lost and the display loop hangs forever.
    //
    // A watch channel always stores the latest value.  `changed()`
    // resolves immediately if the value was already updated (i.e.
    // the child already exited).  The Receiver is Clone, so we can
    // copy it from the immutable DashMap reference.
    let mut exit_rx: Option<tokio::sync::watch::Receiver<bool>> = {
        if let Some(ref id) = active_id {
            match manager.get(id) {
                Some(h) => {
                    // Command still in manager — check if it's already dead.
                    if !h.is_alive() {
                        tracing::info!("Direct child already exited before display loop started");
                        if manager.list().is_empty() {
                            let _ = terminal::disable_raw_mode();
                            let _ = stdout.execute(cursor::Show);
                            let _ = stdout.execute(LeaveAlternateScreen);
                            let _ = shutdown_tx.send(());
                            return true;
                        }
                        if !display_all {
                            dismissed = true;
                        }
                        active_id = None;
                        None
                    } else {
                        // Clone the receiver — watch::Receiver is Clone.
                        let rx = h.exit_rx.clone();
                        // If the child already exited, changed() will resolve
                        // immediately.  But check *value* first as a fast path.
                        if *rx.borrow() {
                            tracing::info!("Direct child already exited (watch flag set)");
                            if manager.list().is_empty() {
                                let _ = terminal::disable_raw_mode();
                                let _ = stdout.execute(cursor::Show);
                                let _ = stdout.execute(LeaveAlternateScreen);
                                let _ = shutdown_tx.send(());
                                return true;
                            }
                            if !display_all {
                                dismissed = true;
                            }
                            active_id = None;
                            None
                        } else {
                            Some(rx)
                        }
                    }
                }
                None => {
                    // Command already removed — child is gone.
                    tracing::info!("Direct child already removed before display loop started");
                    if manager.list().is_empty() {
                        let _ = terminal::disable_raw_mode();
                        let _ = stdout.execute(cursor::Show);
                        let _ = stdout.execute(LeaveAlternateScreen);
                        let _ = shutdown_tx.send(());
                        return true;
                    }
                    if !display_all {
                        dismissed = true;
                    }
                    active_id = None;
                    None
                }
            }
        } else {
            None
        }
    };

    // ── Periodic tick channel (render only, no exit check) ──
    let (tick_tx, mut tick_rx) = mpsc::channel::<()>(4);
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_millis(refresh_ms)).await;
            if tick_tx.send(()).await.is_err() {
                break; // receiver dropped — display loop exited
            }
        }
    });

    /// Render the VTTY buffer for the active command, or clear if none.
    /// Also positions a steady (non-blinking) cursor at the VTTY's
    /// logical cursor position.
    async fn render_vtty(
        manager: &Arc<CommandManager>,
        active_id: &Option<String>,
    ) {
        let commands = manager.list();
        let target_id = active_id.as_ref()
            .or_else(|| commands.first().map(|(id, _, _, _, _)| id));

        if let Some(ref id) = target_id {
            if let Some(handle) = manager.get(id) {
                let buf = handle.vtty_snapshot().await;
                let (cur_row, cur_col) = handle.cursor_position().await;
                drop(handle);
                let _ = TerminalDisplay::render(&buf);
                let _ = TerminalDisplay::show_cursor_at(cur_row, cur_col);
            }
        } else {
            let _ = TerminalDisplay::clear();
        }
    }

    'outer: loop {
        tokio::select! {
            biased;

            // ── Immediate exit notification ──
            _ = async {
                if let Some(rx) = exit_rx.as_mut() {
                    let _ = rx.changed().await;
                } else {
                    std::future::pending::<()>().await;
                }
            } => {
                tracing::info!("Direct child process exited (channel)");
                // Check if other commands remain before deciding to shut down.
                if manager.list().is_empty() {
                    let _ = TerminalDisplay::clear();
                    break 'outer;
                } else if display_all {
                    // Stay in display, show first available command.
                    tracing::info!("Other commands remain; switching to monitor mode");
                    active_id = None;
                    exit_rx = None;
                } else {
                    // Display was only for the CLI command — dismiss it.
                    tracing::info!("Direct CLI command exited; dismissing display (commands remain)");
                    dismissed = true;
                    active_id = None;
                    exit_rx = None;
                }
            }

            // ── Periodic VTTY render ──
            _ = tick_rx.recv() => {
                // ── Dismissed state: show status message, skip VTTY ──
                if dismissed {
                    if !dismiss_rendered {
                        dismiss_rendered = true;
                        let remaining = manager.list();
                        let _ = TerminalDisplay::clear();
                        let _ = write!(stdout, "\r\n\r\n  vrunner: primary command exited. {} command(s) still running.\r\n", remaining.len());
                        for (id, name, _args, pid, _cert) in &remaining {
                            let _ = write!(stdout, "    PID {} — {} ({})\r\n", pid, name, &id[..id.len().min(8)]);
                        }
                        let _ = write!(stdout, "\r\n  Press 'q' or Ctrl+C to shut down.\r\n");
                        let _ = stdout.flush();
                    }
                    // Still check if all commands disappeared via external stop.
                    if manager.list().is_empty() {
                        break 'outer;
                    }
                    continue;
                }
                // Fallback exit detection for the direct child.
                if let Some(ref id) = active_id {
                    let gone = match manager.get(id) {
                        Some(h) => !h.is_alive(),
                        None => true,
                    };
                    if gone {
                        tracing::info!("Direct child process exited (tick fallback)");
                        if manager.list().is_empty() {
                            let _ = TerminalDisplay::clear();
                            break 'outer;
                        } else if display_all {
                            tracing::info!("Other commands remain; switching to monitor mode");
                            active_id = None;
                            exit_rx = None;
                        } else {
                            tracing::info!("Direct CLI command exited; dismissing display (commands remain)");
                            dismissed = true;
                            active_id = None;
                            exit_rx = None;
                        }
                    }
                }
                // For API-spawned commands (no direct child), check if all
                // commands have been removed.
                if exit_rx.is_none() && manager.list().is_empty() {
                    break;
                }
                render_vtty(&manager, &active_id).await;
            }

            // ── SIGWINCH — terminal resize ──
            _ = winch_rx.recv() => {
                if let Some((rows, cols)) = detect_terminal_size() {
                    tracing::debug!(rows, cols, "SIGWINCH: terminal resized");
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
            // Read from /dev/tty via AsyncFd (no blocking thread — clean
            // exit) and forward directly via handle.send_bytes().await
            // (proven working path — NOT tokio::spawn).
            result = tty_async.readable() => {
                match result {
                    Ok(mut guard) => {
                        loop {
                            match guard.try_io(|inner| {
                                use std::io::Read;
                                inner.get_ref().read(&mut stdin_buf)
                            }) {
                                Ok(Ok(0)) => break,  // EOF
                                Ok(Ok(1)) => {
                                    let b = stdin_buf[0];
                                    if b == 0x1c {
                                        break 'outer;  // Ctrl+\ — quit display
                                    }
                                    if dismissed {
                                        if b == b'q' || b == 0x03 {
                                            break 'outer;  // q or Ctrl+C — shut down
                                        }
                                        continue;  // ignore other keys when dismissed
                                    }
                                    // Forward to the active command, or fall back
                                    // to the first available command in monitor mode.
                                    let target_id = if let Some(ref id) = active_id {
                                        Some(id.clone())
                                    } else {
                                        manager.list().first().map(|(id, _, _, _, _)| id.clone())
                                    };
                                    if let Some(ref tid) = target_id {
                                        if let Some(handle) = manager.get(tid) {
                                            let _ = handle.send_bytes(vec![b]).await;
                                        }
                                    }
                                }
                                Ok(Ok(_)) => {}  // ignore >1 byte (shouldn't happen with 1-byte buf)
                                Ok(Err(_)) => { guard.clear_ready(); break; }
                                Err(_would_block) => { guard.clear_ready(); break; }
                            }
                        }
                    }
                    Err(_) => break,
                }
            }

            // ── External shutdown ──
            _ = shutdown_rx.recv() => {
                break 'outer;
            }
        }
    }

    // Restore the terminal before returning.
    // Flush to ensure all queued crossterm commands are applied.
    let _ = stdout.flush();
    let _ = stdout.execute(cursor::Show);
    let _ = stdout.execute(LeaveAlternateScreen);
    let _ = stdout.flush();
    let _ = terminal::disable_raw_mode();
    let _ = stdout.flush();

    // If we broke out of the loop, always trigger shutdown.
    // (The dismissed path now waits for q/Ctrl+C inside the loop.)
    let _ = shutdown_tx.send(());
    true
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
        let cmd_pid = result["data"]["pid"].as_u64().unwrap_or(0);
        let cmd_id = result["data"]["id"].as_str().unwrap_or("?");
        println!("Command spawned successfully on instance {} (PID {})", info.pid, info.pid);
        println!("  PID:       {}", cmd_pid);
        println!("  VTTY:      {}/api/commands/{}/vtty/html", url, cmd_id);
    } else {
        let error = result["error"].as_str().unwrap_or("Unknown error");
        eprintln!("Failed to spawn command: {}", error);
        std::process::exit(1);
    }

    Ok(())
}

/// Handle the `vrunner freeze` subcommand.
async fn handle_freeze_command(cli: &Cli, pid: u32) -> Result<()> {
    let registry = InstanceRegistry::new()?;
    let info = resolve_instance(cli, &registry)?;
    let url = instance_url(&info, &None);
    let client = reqwest::Client::new();

    // Look up the command ID by PID via the instance's API
    let cmd_id = resolve_pid_to_id(&client, &url, pid).await?;

    let resp = client
        .post(format!("{}/api/commands/{}/freeze", url, cmd_id))
        .send()
        .await?;

    let status = resp.status();
    let result: serde_json::Value = resp.json().await?;

    if status.is_success() {
        println!("Command with PID {} frozen (SIGSTOP) on instance {}", pid, info.pid);
    } else {
        let error = result["error"].as_str().unwrap_or("Unknown error");
        eprintln!("Failed to freeze command: {}", error);
        std::process::exit(1);
    }

    Ok(())
}

/// Handle the `vrunner thaw` subcommand.
async fn handle_thaw_command(cli: &Cli, pid: u32) -> Result<()> {
    let registry = InstanceRegistry::new()?;
    let info = resolve_instance(cli, &registry)?;
    let url = instance_url(&info, &None);
    let client = reqwest::Client::new();

    // Look up the command ID by PID via the instance's API
    let cmd_id = resolve_pid_to_id(&client, &url, pid).await?;

    let resp = client
        .post(format!("{}/api/commands/{}/thaw", url, cmd_id))
        .send()
        .await?;

    let status = resp.status();
    let result: serde_json::Value = resp.json().await?;

    if status.is_success() {
        println!("Command with PID {} thawed (SIGCONT) on instance {}", pid, info.pid);
    } else {
        let error = result["error"].as_str().unwrap_or("Unknown error");
        eprintln!("Failed to thaw command: {}", error);
        std::process::exit(1);
    }

    Ok(())
}

/// Handle the `vrunner resize-command` subcommand.
///
/// Resizes the VTTY of a running command by PID or name.
/// Resizes both the in-memory buffer and the child PTY (sends SIGWINCH).
/// If rows/cols are 0 (default), uses the current terminal size.
async fn handle_resize_command(cli: &Cli, target: &str, rows: u16, cols: u16) -> Result<()> {
    let registry = InstanceRegistry::new()?;
    let instances = registry.list_instances();

    if instances.is_empty() {
        anyhow::bail!("No running vrunner instances found. Start one first with: vrunner -- <command>");
    }

    // If rows/cols are 0 (default), detect from the current terminal.
    let (rows, cols) = if rows == 0 || cols == 0 {
        match detect_terminal_size() {
            Some((r, c)) => {
                let r = if rows == 0 { r } else { rows };
                let c = if cols == 0 { c } else { cols };
                (r, c)
            }
            None => {
                let r = if rows == 0 { 24 } else { rows };
                let c = if cols == 0 { 80 } else { cols };
                (r, c)
            }
        }
    } else {
        (rows, cols)
    };

    let client = reqwest::Client::new();

    // Fast path: if target is a pure number, treat as PID.
    if let Ok(pid) = target.parse::<u32>() {
        return handle_resize_by_pid(&client, &instances, pid, rows, cols).await;
    }

    // Collect all commands from all instances (same logic as stop-command).
    let mut all_commands: Vec<(u32, String, u32, String, String)> = Vec::new();
    for info in &instances {
        let url = instance_url(info, &None);
        let resp = client
            .get(format!("{}/api/commands", url))
            .send()
            .await;

        if let Ok(resp) = resp {
            if let Ok(json) = resp.json::<serde_json::Value>().await {
                if let Some(cmds) = json["data"].as_array() {
                    for cmd in cmds {
                        let name = cmd.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let args = cmd.get("args").and_then(|v| v.as_array());
                        let cmd_pid = cmd.get("pid").and_then(|v| v.as_u64()).unwrap_or(0) as u32;

                        let full = match args {
                            Some(arr) => {
                                let arg_strs: Vec<&str> = arr.iter().filter_map(|v| v.as_str()).collect();
                                if arg_strs.is_empty() {
                                    name.clone()
                                } else {
                                    format!("{} {}", name, arg_strs.join(" "))
                                }
                            }
                            None => name.clone(),
                        };

                        if let Some(id) = cmd.get("id").and_then(|v| v.as_str()) {
                            all_commands.push((info.pid, id.to_string(), cmd_pid, name, full));
                        }
                    }
                }
            }
        }
    }

    if all_commands.is_empty() {
        anyhow::bail!("No running commands found. Use `vrunner list` to see running commands.");
    }

    // Exact match on name alone or full "name args" string.
    let exact: Vec<_> = all_commands.iter()
        .filter(|(_, _, _, name, full)| name == target || full == target)
        .collect();

    if exact.len() == 1 {
        let (inst_pid, ref cmd_id, cmd_pid, _, _) = exact[0];
        let info = instances.iter().find(|i| i.pid == *inst_pid).unwrap();
        let url = instance_url(info, &None);
        return resize_command_by_id(&client, &url, cmd_id, *cmd_pid, *inst_pid, rows, cols).await;
    }
    if exact.len() > 1 {
        eprintln!("Multiple commands match '{}':", target);
        for (_, _, pid, name, full) in &exact {
            eprintln!("  PID {} — {}", pid, full);
        }
        anyhow::bail!("Ambiguous target. Use PID to disambiguate.");
    }

    // Prefix match on full string, then on name only (same as stop-command).
    let prefix_full: Vec<_> = all_commands.iter()
        .filter(|(_, _, _, _, full)| full.starts_with(target))
        .collect();
    if prefix_full.len() == 1 {
        let (inst_pid, ref cmd_id, cmd_pid, _, _) = prefix_full[0];
        let info = instances.iter().find(|i| i.pid == *inst_pid).unwrap();
        let url = instance_url(info, &None);
        return resize_command_by_id(&client, &url, cmd_id, *cmd_pid, *inst_pid, rows, cols).await;
    }

    let prefix_name: Vec<_> = all_commands.iter()
        .filter(|(_, _, _, name, _)| name.starts_with(target))
        .collect();
    if prefix_name.len() == 1 {
        let (inst_pid, ref cmd_id, cmd_pid, _, _) = prefix_name[0];
        let info = instances.iter().find(|i| i.pid == *inst_pid).unwrap();
        let url = instance_url(info, &None);
        return resize_command_by_id(&client, &url, cmd_id, *cmd_pid, *inst_pid, rows, cols).await;
    }

    anyhow::bail!("No command matching '{}' found. Use `vrunner list` to see running commands.", target);
}

/// Resize a command by its UUID via the instance's HTTP API.
async fn resize_command_by_id(
    client: &reqwest::Client,
    url: &str,
    cmd_id: &str,
    cmd_pid: u32,
    inst_pid: u32,
    rows: u16,
    cols: u16,
) -> Result<()> {
    let resp = client
        .post(format!("{}/api/commands/{}/resize", url, cmd_id))
        .json(&serde_json::json!({ "rows": rows, "cols": cols }))
        .send()
        .await?;

    let status = resp.status();
    let body: serde_json::Value = resp.json().await?;

    if status.is_success() && body.get("status").and_then(|s| s.as_str()) == Some("ok") {
        println!("Resized command with PID {} to {}x{} on instance {} (PID {})", cmd_pid, rows, cols, inst_pid, inst_pid);
        Ok(())
    } else {
        let err_msg = body.get("error").and_then(|e| e.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("HTTP {}", status));
        anyhow::bail!("Failed to resize command with PID {}: {}", cmd_pid, err_msg);
    }
}

/// Resize a command by its OS PID, trying all running instances.
async fn handle_resize_by_pid(
    client: &reqwest::Client,
    instances: &[vrunner::instance::info::InstanceInfo],
    pid: u32,
    rows: u16,
    cols: u16,
) -> Result<()> {
    for info in instances {
        let url = instance_url(info, &None);
        match resolve_pid_to_id(client, &url, pid).await {
            Ok(cmd_id) => {
                return resize_command_by_id(client, &url, &cmd_id, pid, info.pid, rows, cols).await;
            }
            Err(_) => continue,
        }
    }
    anyhow::bail!("No command found with PID {}. Use `vrunner list` to see running commands.", pid);
}

/// Resolve a PID to a command UUID by querying the instance's command list.
async fn resolve_pid_to_id(
    client: &reqwest::Client,
    url: &str,
    pid: u32,
) -> Result<String> {
    let resp = client
        .get(format!("{}/api/commands", url))
        .send()
        .await?;

    let json: serde_json::Value = resp.json().await?;
    if json["status"] != "ok" {
        anyhow::bail!("Failed to query commands from instance");
    }

    if let Some(cmds) = json["data"].as_array() {
        for cmd in cmds {
            if cmd.get("pid").and_then(|v| v.as_u64()) == Some(pid as u64) {
                if let Some(id) = cmd.get("id").and_then(|v| v.as_str()) {
                    return Ok(id.to_string());
                }
            }
        }
    }

    anyhow::bail!("No command found with PID {}", pid)
}

/// Handle the `vrunner list` subcommand.
///
/// Queries running vrunner instances and shows their commands in a
/// two-level indented hierarchy:
///
///   INSTANCE  PID: 12345  PORT: 9090  BIND: 127.0.0.1  DAEMON: no  DISPLAY: yes
///     COMMAND  htop                              PID: 5678  CERT: -
///     COMMAND  vim file.txt                      PID: 5679  CERT: my-app
///
/// When `--target <PID>` is provided, only that instance is listed.
/// Unreachable instances show an `[ERROR]` line under their header.
/// Instances with no commands show `(no commands)`.
async fn handle_list_command(cli: &Cli) -> Result<()> {
    let registry = InstanceRegistry::new()?;
    let all_instances = registry.list_instances();

    // Resolve target: filter to a single instance if --target is given.
    let instances: Vec<vrunner::instance::info::InstanceInfo> = if let Some(target_pid) = cli.target {
        match all_instances.iter().find(|i| i.pid == target_pid) {
            Some(info) => vec![info.clone()],
            None => {
                if all_instances.is_empty() {
                    anyhow::bail!("No running vrunner instances found.");
                }
                anyhow::bail!(
                    "No vrunner instance found with PID {}. Running instances:\n{}",
                    target_pid,
                    all_instances.iter().map(|i| format!("  PID: {}", i.pid)).collect::<Vec<_>>().join("\n")
                );
            }
        }
    } else {
        all_instances
    };

    if instances.is_empty() {
        println!("No running vrunner instances.");
        return Ok(());
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()?;

    for info in &instances {
        println!("{}", format_instance_header(info));

        let url = instance_url(info, &None);

        match client.get(format!("{}/api/commands", url)).send().await {
            Ok(resp) => {
                match resp.json::<serde_json::Value>().await {
                    Ok(json) => {
                        if json["status"] == "ok" {
                            if let Some(cmds) = json["data"].as_array() {
                                if cmds.is_empty() {
                                    println!("  {}", c("(no commands)", Color::Yellow, false));
                                } else {
                                    for cmd in cmds {
                                        if let Some(line) = format_command(cmd) {
                                            println!("{}", line);
                                        }
                                    }
                                }
                            } else {
                                println!("  {}  Invalid API response: expected array", c("[ERROR]", Color::Red, true));
                            }
                        } else {
                            let err = json["error"].as_str().unwrap_or("unknown error");
                            println!("  {}  API returned error: {}", c("[ERROR]", Color::Red, true), err);
                        }
                    }
                    Err(e) => {
                        println!("  {}  Invalid API response: {}", c("[ERROR]", Color::Red, true), e);
                    }
                }
            }
            Err(e) => {
                println!("  {}  Instance unreachable: {}", c("[ERROR]", Color::Red, true), e);
            }
        }

        // Blank line between instances for readability
        if instances.len() > 1 {
            println!();
        }
    }

    Ok(())
}

/// Format an instance header line for `vrunner list` output.
fn format_instance_header(info: &vrunner::instance::info::InstanceInfo) -> String {
    let daemon = if info.daemon { "yes" } else { "no" };
    let display = if info.display { "yes" } else { "no" };
    format!(
        "{}  {} {}  {} {}  {} {}  {} {}  {} {}",
        c("INSTANCE", Color::Blue, true),
        c("PID:", Color::DarkGrey, false), info.pid,
        c("PORT:", Color::DarkGrey, false), info.port,
        c("BIND:", Color::DarkGrey, false), info.bind,
        c("DAEMON:", Color::DarkGrey, false), daemon,
        c("DISPLAY:", Color::DarkGrey, false), display,
    )
}

/// Format a single command line for `vrunner list` output.
/// Returns None if the JSON value lacks required fields.
fn format_command(cmd: &serde_json::Value) -> Option<String> {
    let name = cmd.get("name")?.as_str()?;
    let args = cmd.get("args")?.as_array()?;
    let args_vec: Vec<&str> = args.iter().filter_map(|v| v.as_str()).collect();
    let display_name = if args_vec.is_empty() {
        name.to_string()
    } else {
        format!("{} {}", name, args_vec.join(" "))
    };
    // Truncate long command names for readability
    let truncated = if display_name.len() > 40 {
        format!("{}...", &display_name[..37])
    } else {
        display_name
    };
    let pid = cmd.get("pid").and_then(|v| v.as_u64()).unwrap_or(0);
    let cert = cmd.get("certificate").and_then(|v| v.as_str()).unwrap_or("-");

    Some(format!(
        "  {} {}  {}",
        c(&format!("{:<10}", pid), Color::Cyan, false),
        c(&format!("{:<20}", truncated), Color::Reset, false),
        c(&format!("CERT: {}", cert), Color::DarkGrey, false),
    ))
}

/// Stop a specific command by PID or name on any running instance.
///
/// If `target` parses as a u32, it is treated as a PID and resolved
/// via `resolve_pid_to_id` (same as freeze/thaw).
///
/// If `target` is a name (or "name args..."), matching proceeds in three
/// rounds with increasing looseness.  A match from an earlier round wins:
///   1. Exact: `name == target` or `name arg1 arg2 ... == target`
///   2. Prefix on full: `name arg1 arg2 ...` starts with `target`
///   3. Prefix on name: `name` starts with `target`
/// If after all rounds exactly one command matches, it is stopped.
/// If multiple commands match, an error lists them and suggests using a
/// PID to disambiguate.
///
/// Returns true if exactly one command was found and stopped.
async fn handle_stop_command(_cli: &Cli, target: &str) -> Result<bool> {
    let registry = InstanceRegistry::new()?;
    let instances = registry.list_instances();

    if instances.is_empty() {
        return Ok(false);
    }

    let client = reqwest::Client::new();

    // Fast path: if target is a pure number, treat as PID.
    if let Ok(pid) = target.parse::<u32>() {
        return handle_stop_command_by_pid_on_instances(&client, &instances, pid).await;
    }

    // Collect all commands from all instances.
    // Each entry: (instance_pid, cmd_id, cmd_pid, name, full_display)
    let mut all_commands: Vec<(u32, String, u32, String, String)> = Vec::new();
    for info in &instances {
        let url = instance_url(info, &None);
        let resp = client
            .get(format!("{}/api/commands", url))
            .send()
            .await;

        if let Ok(resp) = resp {
            if let Ok(json) = resp.json::<serde_json::Value>().await {
                if let Some(cmds) = json["data"].as_array() {
                    for cmd in cmds {
                        let name = cmd.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let args = cmd.get("args").and_then(|v| v.as_array());
                        let cmd_pid = cmd.get("pid").and_then(|v| v.as_u64()).unwrap_or(0) as u32;

                        let full = match args {
                            Some(arr) => {
                                let arg_strs: Vec<&str> = arr.iter().filter_map(|v| v.as_str()).collect();
                                if arg_strs.is_empty() {
                                    name.clone()
                                } else {
                                    format!("{} {}", name, arg_strs.join(" "))
                                }
                            }
                            None => name.clone(),
                        };

                        if let Some(id) = cmd.get("id").and_then(|v| v.as_str()) {
                            all_commands.push((info.pid, id.to_string(), cmd_pid, name, full));
                        }
                    }
                }
            }
        }
    }

    if all_commands.is_empty() {
        return Ok(false);
    }

    // Round 1: exact match on name alone or full "name args" string.
    let exact: Vec<_> = all_commands.iter()
        .filter(|(_, _, _, name, full)| name == target || full == target)
        .collect();

    if exact.len() == 1 {
        let (inst_pid, ref cmd_id, cmd_pid, _, _) = exact[0];
        let info = instances.iter().find(|i| i.pid == *inst_pid).unwrap();
        let url = instance_url(info, &None);
        return stop_command_by_id(&client, &url, cmd_id, *cmd_pid, *inst_pid).await;
    }
    if exact.len() > 1 {
        eprintln!("Multiple commands match '{}':", target);
        for (inst_pid, _, cmd_pid, _, full) in &exact {
            eprintln!("  PID {} — {} (on instance {})", cmd_pid, full, inst_pid);
        }
        eprintln!("Use a PID to disambiguate.");
        return Ok(false);
    }

    // Round 2: prefix match on full "name args" string.
    let prefix_full: Vec<_> = all_commands.iter()
        .filter(|(_, _, _, _, full)| full.starts_with(target))
        .collect();

    if prefix_full.len() == 1 {
        let (inst_pid, ref cmd_id, cmd_pid, _, _) = prefix_full[0];
        let info = instances.iter().find(|i| i.pid == *inst_pid).unwrap();
        let url = instance_url(info, &None);
        return stop_command_by_id(&client, &url, cmd_id, *cmd_pid, *inst_pid).await;
    }
    if prefix_full.len() > 1 {
        eprintln!("Multiple commands match '{}':", target);
        for (inst_pid, _, cmd_pid, _, full) in &prefix_full {
            eprintln!("  PID {} — {} (on instance {})", cmd_pid, full, inst_pid);
        }
        eprintln!("Use a longer prefix or a PID to disambiguate.");
        return Ok(false);
    }

    // Round 3: prefix match on name alone.
    let prefix_name: Vec<_> = all_commands.iter()
        .filter(|(_, _, _, name, _)| name.starts_with(target))
        .collect();

    if prefix_name.len() == 1 {
        let (inst_pid, ref cmd_id, cmd_pid, _, _) = prefix_name[0];
        let info = instances.iter().find(|i| i.pid == *inst_pid).unwrap();
        let url = instance_url(info, &None);
        return stop_command_by_id(&client, &url, cmd_id, *cmd_pid, *inst_pid).await;
    }
    if prefix_name.len() > 1 {
        eprintln!("Multiple commands match '{}':", target);
        for (inst_pid, _, cmd_pid, _, full) in &prefix_name {
            eprintln!("  PID {} — {} (on instance {})", cmd_pid, full, inst_pid);
        }
        eprintln!("Use a longer prefix or a PID to disambiguate.");
        return Ok(false);
    }

    // No match at all.
    Ok(false)
}

/// Internal: send the kill request for a resolved command ID.
async fn stop_command_by_id(
    client: &reqwest::Client,
    url: &str,
    cmd_id: &str,
    cmd_pid: u32,
    inst_pid: u32,
) -> Result<bool> {
    let resp = client
        .post(format!("{}/api/commands/{}/kill", url, cmd_id))
        .json(&serde_json::json!({}))
        .send()
        .await;

    match resp {
        Ok(resp) => {
            let status = resp.status();
            let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::json!({"status": "unknown"}));
            if status.is_success() && body.get("status").and_then(|s| s.as_str()) == Some("ok") {
                println!("Command with PID {} stopped on instance {} (PID {})", cmd_pid, inst_pid, inst_pid);
                Ok(true)
            } else {
                let err_msg = body.get("error").and_then(|e| e.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| format!("HTTP {}", status));
                eprintln!("Failed to stop command with PID {}: {}", cmd_pid, err_msg);
                Ok(false)
            }
        }
        Err(e) => {
            eprintln!("Failed to stop command with PID {}: {}", cmd_pid, e);
            Ok(false)
        }
    }
}

/// Internal: stop a command by PID on a list of instances.
/// Used by handle_stop_command when target parses as a number.
async fn handle_stop_command_by_pid_on_instances(
    client: &reqwest::Client,
    instances: &[vrunner::instance::info::InstanceInfo],
    pid: u32,
) -> Result<bool> {
    for info in instances {
        let url = instance_url(info, &None);
        let cmd_id = match resolve_pid_to_id(client, &url, pid).await {
            Ok(id) => id,
            Err(_) => continue,
        };

        let resp = client
            .post(format!("{}/api/commands/{}/kill", url, cmd_id))
            .json(&serde_json::json!({}))
            .send()
            .await;

        match resp {
            Ok(resp) => {
                let status = resp.status();
                let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::json!({"status": "unknown"}));
                if status.is_success() && body.get("status").and_then(|s| s.as_str()) == Some("ok") {
                    println!("Command with PID {} stopped on instance {} (PID {})", pid, info.pid, info.pid);
                    return Ok(true);
                } else {
                    let err_msg = body.get("error").and_then(|e| e.as_str())
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| format!("HTTP {}", status));
                    eprintln!("Failed to stop command with PID {}: {}", pid, err_msg);
                    return Ok(false);
                }
            }
            Err(e) => {
                eprintln!("Failed to stop command with PID {}: {}", pid, e);
                return Ok(false);
            }
        }
    }

    Ok(false)
}

/// Filter instances by --target, returning all if no target specified.
fn resolve_targeted_instances(
    cli: &Cli,
    all_instances: &[vrunner::instance::info::InstanceInfo],
) -> Result<Vec<vrunner::instance::info::InstanceInfo>> {
    if let Some(target_pid) = cli.target {
        match all_instances.iter().find(|i| i.pid == target_pid) {
            Some(info) => Ok(vec![info.clone()]),
            None => {
                if all_instances.is_empty() {
                    anyhow::bail!("No running vrunner instances found.");
                }
                anyhow::bail!(
                    "No vrunner instance found with PID {}. Running instances:\n{}",
                    target_pid,
                    all_instances.iter().map(|i| format!("  PID: {}", i.pid)).collect::<Vec<_>>().join("\n")
                );
            }
        }
    } else {
        Ok(all_instances.to_vec())
    }
}

/// Handle the `vrunner list-vrunner` subcommand.
///
/// Lists vrunner instances in tab-separated (TSV) format for machine parsing.
async fn handle_list_vrunner_command(cli: &Cli) -> Result<()> {
    let registry = InstanceRegistry::new()?;
    let all_instances = registry.list_instances();
    let instances = resolve_targeted_instances(cli, &all_instances)?;

    // Print TSV header
    println!("PID\tPORT\tBIND\tDAEMON\tDISPLAY\tSTARTUP_CMD");
    for info in &instances {
        let startup = info.command.as_deref().unwrap_or("(idle)");
        let daemon = if info.daemon { "yes" } else { "no" };
        let display = if info.display { "yes" } else { "no" };
        println!("{}\t{}\t{}\t{}\t{}\t{}",
            info.pid, info.port, info.bind, daemon, display, startup);
    }
    Ok(())
}

/// Handle the `vrunner list-commands` subcommand.
///
/// Lists all running commands across instances in tab-separated (TSV) format.
async fn handle_list_commands_command(cli: &Cli) -> Result<()> {
    let registry = InstanceRegistry::new()?;
    let all_instances = registry.list_instances();
    let instances = resolve_targeted_instances(cli, &all_instances)?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()?;

    // Print TSV header
    println!("VRUNNER_PID\tCMD_PID\tNAME\tARGS\tCERT");

    for info in &instances {
        let url = instance_url(info, &None);
        match client.get(format!("{}/api/commands", url)).send().await {
            Ok(resp) => {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    if let Some(cmds) = json["data"].as_array() {
                        for cmd in cmds {
                            let cmd_pid = cmd.get("pid").and_then(|v| v.as_u64()).unwrap_or(0);
                            let name = cmd.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                            let args = serde_json::to_string(cmd.get("args").unwrap_or(&serde_json::json!([]))).unwrap_or_else(|_| "[]".to_string());
                            let cert = cmd.get("certificate").and_then(|v| v.as_str()).unwrap_or("-");
                            println!("{}\t{}\t{}\t{}\t{}",
                                info.pid, cmd_pid, name, args, cert);
                        }
                    }
                }
            }
            Err(_) => {
                // Skip unreachable instances silently in TSV mode
            }
        }
    }
    Ok(())
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
