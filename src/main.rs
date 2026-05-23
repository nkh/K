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
        Some(Commands::StopCommand { pid: _ }) => {
            // stop-command is async (needs HTTP), fall through to async phase
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
    if let Some(Commands::StopCommand { pid }) = cli.command {
        let stopped = handle_stop_command_by_pid(&cli, pid).await?;
        if !stopped {
            eprintln!("No command found with PID {}. Use `vrunner list` to see running commands.", pid);
            std::process::exit(1);
        }
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
        // No display, but a child was spawned directly via CLI.  Forward
        // the parent's stdin to the child's PTY so keyboard input reaches
        // the command, and wait for the child to exit before shutting down.
        run_headless(&manager, id).await;
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
    shutdown_tx: broadcast::Sender<()>,
) {
    use vrunner::vtty::display::TerminalDisplay;
    use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
    use crossterm::{cursor, ExecutableCommand};
    use std::io::Write;

    // Set up the alternate screen and raw mode.
    let mut stdout = std::io::stdout();
    if let Err(e) = terminal::enable_raw_mode() {
        tracing::warn!(error = %e, "Failed to enable raw mode");
        return;
    }
    let _ = stdout.execute(EnterAlternateScreen);
    let _ = stdout.execute(cursor::Hide);

    // ── Truly async keystroke reading via /dev/tty ──
    //
    // We do NOT use tokio::io::stdin().  Despite its name, tokio's Stdin
    // wraps the synchronous std::io::Stdin in a spawn_blocking thread.
    // That thread calls the real stdin.read(), which is impossible to cancel.
    // When the display loop breaks on child exit, the future is dropped but
    // the blocking thread stays stuck on stdin.read().  During Runtime::drop,
    // tokio waits for all blocking threads to complete → the process hangs
    // until the user presses Enter (which unblocks the read).
    //
    // Instead, we open /dev/tty and wrap it in tokio::io::unix::AsyncFd.
    // AsyncFd uses mio (edge-triggered, level-aware) to register the fd with
    // the I/O reactor.  When the future is dropped (select! picks another
    // branch), the registration is removed — no thread, no hang.
    //
    // We use /dev/tty (not stdin fd 0) because:
    //   1. /dev/tty always refers to the controlling terminal, even if
    //      stdin has been redirected (e.g. piped input).
    //   2. It is the only way to reliably get keystrokes in raw mode
    //      after crossterm::terminal::enable_raw_mode() has been called.
    let tty_async: tokio::io::unix::AsyncFd<std::fs::File> = {
        let tty = match std::fs::File::open("/dev/tty") {
            Ok(f) => f,
            Err(e) => {
                tracing::error!(error = %e, "Failed to open /dev/tty");
                let _ = terminal::disable_raw_mode();
                let _ = stdout.execute(cursor::Show);
                let _ = stdout.execute(LeaveAlternateScreen);
                return;
            }
        };
        match tokio::io::unix::AsyncFd::new(tty) {
            Ok(a) => a,
            Err(e) => {
                tracing::error!(error = %e, "Failed to create AsyncFd for /dev/tty");
                let _ = terminal::disable_raw_mode();
                let _ = stdout.execute(cursor::Show);
                let _ = stdout.execute(LeaveAlternateScreen);
                return;
            }
        }
    };

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
    // Track which command we're displaying so we can forward keystrokes.
    let active_id: Option<String> = direct_child_id.map(String::from);

    // Helper: resolve the command ID that should receive keystrokes.
    // Uses the same fallback as render_vtty: prefer the direct child,
    // otherwise fall back to the first available command.
    fn resolve_input_target(
        manager: &Arc<CommandManager>,
        active_id: &Option<String>,
    ) -> Option<String> {
        active_id.clone().or_else(|| {
            manager.list().first().map(|(id, _, _, _, _)| id.clone())
        })
    }

    // ── Event-driven exit detection ──
    // Instead of polling with waitpid or periodic ticks, we use a
    // tokio::sync::Notify that the process waiter fires immediately
    // when child.wait() returns.  This makes exit detection instant —
    // zero delay, zero polling, zero race conditions.
    let exit_notify: Option<Arc<tokio::sync::Notify>> = {
        if let Some(ref id) = active_id {
            manager.get(id).map(|h| h.exit_notify.clone())
        } else {
            None
        }
    };

    // Helper future: await the exit notify, or hang forever if None.
    async fn await_exit(notify: Option<&Arc<tokio::sync::Notify>>) {
        if let Some(n) = notify {
            n.notified().await;
        } else {
            // No direct child — wait forever (API-spawned commands
            // are tracked via the periodic tick below).
            std::future::pending::<()>().await;
        }
    }

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

    let mut should_break;
    loop {
        should_break = false;
        tokio::select! {
            // Use biased mode so the exit-notify branch is preferred when
            // multiple branches are ready simultaneously.  This minimises
            // the window where a Notify permit can be lost (see comment
            // in the tick handler below).
            biased;

            // ── Immediate exit notification ──
            // Fires the instant child.wait() returns in the process waiter.
            _ = await_exit(exit_notify.as_ref()) => {
                tracing::info!("Child process exited (notify), shutting down");
                let _ = TerminalDisplay::clear();
                break;
            }

            // ── Periodic VTTY render ──
            _ = tick_rx.recv() => {
                // Fallback exit detection for the direct child.
                //
                // Rationale: tokio::sync::Notify::notify_waiters() wakes
                // all *current* waiters but does NOT store a permit for
                // future ones.  If the select! picks a different branch
                // (tick / stdin) while the notified() future was ready,
                // the future is dropped and the notification is lost —
                // the next notified() call will hang forever.
                //
                // The biased; keyword above makes this unlikely, but
                // not impossible (e.g. both branches become ready in
                // the same poll cycle).  This fallback catches the edge
                // case within one refresh cycle.
                if let Some(ref id) = active_id {
                    let gone = match manager.get(id) {
                        Some(h) => !h.is_alive(),
                        None => true,
                    };
                    if gone {
                        tracing::info!("Child process exited (tick fallback), shutting down");
                        let _ = TerminalDisplay::clear();
                        break;
                    }
                }
                // For API-spawned commands (no direct child), check if all
                // commands have been removed.
                if exit_notify.is_none() && manager.list().is_empty() {
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

            // ── Keystroke forwarding via AsyncFd (truly async, no blocking thread) ──
            result = tty_async.readable() => {
                match result {
                    Ok(mut guard) => {
                        // Drain all available bytes from /dev/tty.
                        // In raw mode each syscall returns one byte,
                        // but we loop until EAGAIN to handle bursts.
                        let mut key_buf = [0u8; 1];
                        loop {
                            match guard.try_io(|inner| {
                                use std::io::Read;
                                inner.get_ref().read(&mut key_buf)
                            }) {
                                Ok(Ok(1)) => {
                                    let b = key_buf[0];
                                    if b == 0x1c {
                                        should_break = true;  // Ctrl+\
                                        break;
                                    }
                                    if let Some(ref target) = resolve_input_target(&manager, &active_id) {
                                        if let Some(handle) = manager.get(target) {
                                            let _ = handle.send_bytes(vec![b]).await;
                                        }
                                    }
                                }
                                Ok(Ok(_)) => { should_break = true; break; }  // EOF
                                Ok(Err(_)) => { should_break = true; break; }  // Read error
                                Err(_would_block) => {
                                    // No more data — clear readiness and
                                    // wait for the next keystroke.
                                    guard.clear_ready();
                                    break;
                                }
                            }
                        }
                    }
                    Err(_) => { should_break = true; }
                }
            }

            // ── External shutdown ──
            _ = shutdown_rx.recv() => {
                break;
            }
        }
        if should_break {
            break;
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

    // Trigger server shutdown so async_main can unwind.
    let _ = shutdown_tx.send(());
}

/// Run in headless mode: forward the parent's stdin to the child's PTY and
/// wait for the child process to exit.
///
/// This is the headless counterpart of `run_display_loop`.  It puts the
/// parent terminal in raw mode, opens /dev/tty for keystroke reading, and
/// forwards every byte to the child via `handle.send_bytes()`.  It also
/// propagates SIGWINCH to the child's PTY so terminal-resize-aware programs
/// (vim, htop, etc.) adjust their layout.
///
/// Exit triggers:
///   1. Child process exits (via exit_notify or periodic poll fallback).
///   2. stdin reaches EOF (e.g. redirected input is exhausted).
///   3. An I/O error on /dev/tty.
async fn run_headless(
    manager: &Arc<CommandManager>,
    child_id: &str,
) {
    use crossterm::terminal;

    // Put the parent terminal in raw mode so keystrokes are delivered
    // immediately (no line buffering).  This is essential for interactive
    // programs like vim, htop, less, etc.
    if let Err(e) = terminal::enable_raw_mode() {
        tracing::warn!(error = %e, "Failed to enable raw mode for headless stdin forwarding");
        // Fall back to the old polling-only behaviour (no stdin forwarding).
        wait_for_child(manager, child_id).await;
        return;
    }

    // Open /dev/tty for reading — same rationale as in run_display_loop:
    // always refers to the controlling terminal even if stdin is redirected.
    let tty = match std::fs::File::open("/dev/tty") {
        Ok(f) => f,
        Err(e) => {
            tracing::error!(error = %e, "Failed to open /dev/tty for headless mode");
            let _ = terminal::disable_raw_mode();
            return;
        }
    };

    let tty_async = match tokio::io::unix::AsyncFd::new(tty) {
        Ok(a) => a,
        Err(e) => {
            tracing::error!(error = %e, "Failed to create AsyncFd for /dev/tty");
            let _ = terminal::disable_raw_mode();
            return;
        }
    };

    // Set up SIGWINCH handler for terminal resize propagation.
    let mut winch_rx = {
        use tokio::signal::unix::{signal, SignalKind};
        match signal(SignalKind::window_change()) {
            Ok(stream) => stream,
            Err(_) => {
                // No WINCH handling — not critical in headless mode.
                let (_tx, rx) = tokio::sync::mpsc::channel::<()>(1);
                drop(rx);
                return;
            }
        }
    };

    // Event-driven exit detection via Notify (same pattern as display loop).
    let child_id_owned = child_id.to_string();
    let exit_notify: Option<Arc<tokio::sync::Notify>> = {
        manager.get(&child_id_owned).map(|h| h.exit_notify.clone())
    };

    // Periodic tick for fallback exit detection (same reason as display loop:
    // Notify permits can be lost if select! picks a different branch).
    let (tick_tx, mut tick_rx) = tokio::sync::mpsc::channel::<()>(4);
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            if tick_tx.send(()).await.is_err() {
                break;
            }
        }
    });

    loop {
        let mut should_break = false;
        tokio::select! {
            biased;

            // ── Immediate exit notification ──
            _ = async {
                if let Some(n) = exit_notify.as_ref() {
                    n.notified().await;
                } else {
                    std::future::pending::<()>().await;
                }
            } => {
                tracing::info!("Child process exited (notify)");
                break;
            }

            // ── Periodic fallback exit check ──
            _ = tick_rx.recv() => {
                let gone = match manager.get(&child_id_owned) {
                    Some(h) => !h.is_alive(),
                    None => true,
                };
                if gone {
                    tracing::info!("Child process exited (tick fallback)");
                    break;
                }
            }

            // ── SIGWINCH — propagate resize to child PTY ──
            _ = winch_rx.recv() => {
                if let Some((rows, cols)) = detect_terminal_size() {
                    if let Some(handle) = manager.get(&child_id_owned) {
                        if let Err(e) = handle.resize_pty(rows, cols).await {
                            tracing::warn!(error = %e, "Failed to resize child PTY on WINCH");
                        }
                    }
                }
            }

            // ── Keystroke forwarding ──
            result = tty_async.readable() => {
                match result {
                    Ok(mut guard) => {
                        let mut key_buf = [0u8; 1];
                        loop {
                            match guard.try_io(|inner| {
                                use std::io::Read;
                                inner.get_ref().read(&mut key_buf)
                            }) {
                                Ok(Ok(1)) => {
                                    if let Some(handle) = manager.get(&child_id_owned) {
                                        let _ = handle.send_bytes(vec![key_buf[0]]).await;
                                    } else {
                                        // Command removed — child gone
                                        should_break = true;
                                        break;
                                    }
                                }
                                Ok(Ok(_)) => { should_break = true; break; } // EOF
                                Ok(Err(_)) => { should_break = true; break; } // Read error
                                Err(_would_block) => {
                                    guard.clear_ready();
                                    break;
                                }
                            }
                        }
                    }
                    Err(_) => { should_break = true; }
                }
            }
        }
        if should_break {
            break;
        }
    }

    // Restore the terminal to its original cooked mode.
    let _ = terminal::disable_raw_mode();
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
