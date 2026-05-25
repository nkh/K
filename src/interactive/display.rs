//! Interactive terminal display loop.
//!
//! This module contains the core display loop that renders VTTY buffers to
//! the local terminal, forwards keystrokes to child commands, and handles
//! terminal resize events (SIGWINCH). It is the primary interactive interface
//! for vrunner's `--display` mode.
//!
//! The display loop runs entirely within the async runtime and uses crossterm
//! for terminal control, AsyncFd for non-blocking keystroke reading from
//! /dev/tty, and tokio channels for event coordination.

use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};

use crate::process::manager::CommandManager;

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
pub async fn run_display_loop(
    manager: &Arc<CommandManager>,
    direct_child_id: Option<&str>,
    refresh_ms: u64,
    display_all: bool,
    shutdown_tx: broadcast::Sender<()>,
    keybindings: &crate::config::schema::KeybindingsConfig,
    log_entries: &Arc<std::sync::Mutex<Vec<String>>>,
    show_tabs: bool,
) -> bool {
    use crate::interactive::{Binding, Action, ActionEffect, check_bindings, resolve_keybindings};
    use crate::interactive::{render_help_overlay, read_spawn_command, restore_raw_mode};
    use crate::vtty::display::TerminalDisplay;
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

    // ── Escape sequence buffer for keybinding matching ──
    // When we receive ESC (0x1b), we start buffering subsequent bytes to form
    // a complete escape sequence (e.g., ESC [ 1 ; 5 D for Ctrl+Left).
    // We then check if the accumulated bytes match any configured keybinding.
    // If they do, we execute the action; if not, we forward all bytes to the
    // active command.  Buffer is consumed after each match attempt or timeout.
    let mut esc_buf: Vec<u8> = Vec::with_capacity(16);
    let mut esc_deadline: Option<tokio::time::Instant> = None;
    const ESC_TIMEOUT_MS: u64 = 50; // max ms to wait for complete escape sequence

    // Build the keybinding lookup table using the interactive module.
    // Accepts both human-readable names ("ctrl+right") and raw escapes.
    let bindings: Vec<Binding> = resolve_keybindings(keybindings);

    // State for overlays
    let mut showing_log = false;
    let mut showing_help = false;
    let mut log_scroll_offset: usize = 0;

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
    // Save the direct child's ID so we can distinguish it from
    // commands spawned via F12 or the web UI.
    let direct_child_owned: Option<String> = direct_child_id.map(String::from);

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
                        if !display_all && manager.list().is_empty() {
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
                            if !display_all && manager.list().is_empty() {
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
                    if !display_all && manager.list().is_empty() {
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
        tab_offset: u16,
        display_all: bool,
    ) {
        use crate::vtty::display::TerminalDisplay;

        let commands = manager.list();
        let target_id = active_id.as_ref()
            .or_else(|| commands.first().map(|(id, _, _, _, _)| id));

        if let Some(ref id) = target_id {
            if let Some(handle) = manager.get(id) {
                let buf = handle.vtty_snapshot().await;
                let (cur_row, cur_col) = handle.cursor_position().await;
                drop(handle);
                let _ = TerminalDisplay::render(&buf, tab_offset);
                let _ = TerminalDisplay::show_cursor_at(cur_row + tab_offset as usize, cur_col);
            }
        } else {
            // No active command — in display_all mode show a waiting
            // message instead of a blank screen so the user knows the
            // display is alive and waiting for commands.
            use std::io::Write;
            let mut stdout = std::io::stdout();
            let _ = TerminalDisplay::clear();
            if display_all {
                let _ = write!(stdout, "\r\n  vrunner: no commands running.\r\n");
                let _ = write!(stdout, "  Waiting for commands (web UI, API, or F12 to spawn).\r\n");
                let _ = write!(stdout, "\r\n  Press Ctrl+\\ to quit.\r\n");
                let _ = stdout.flush();
            }
        }
    }

    /// Render a tab bar at the top of the terminal listing all running commands.
    /// The active command is highlighted with reverse video.
    fn render_tab_bar(
        manager: &CommandManager,
        active_id: &Option<String>,
    ) {
        use crossterm::{
            style::{self, Attribute, Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor},
            cursor::MoveTo,
            terminal::ClearType,
            QueueableCommand,
        };
        let mut stdout = std::io::stdout();
        let (phys_cols, _) = crossterm::terminal::size().unwrap_or((80, 24));
        let commands = manager.list();

        // Background for the tab bar
        stdout.queue(SetBackgroundColor(Color::Rgb { r: 40, g: 42, b: 54 })).ok();
        stdout.queue(SetForegroundColor(Color::Rgb { r: 180, g: 180, b: 180 })).ok();
        stdout.queue(MoveTo(0, 0)).ok();
        stdout.queue(crossterm::terminal::Clear(ClearType::UntilNewLine)).ok();

        if commands.is_empty() {
            stdout.queue(Print(" (no commands)")).ok();
            stdout.queue(ResetColor).ok();
            stdout.flush().ok();
            return;
        }

        let mut col: u16 = 1;
        for (id, name, _args, _pid, _cert) in &commands {
            let is_active = active_id.as_ref().map_or(false, |a| a == id);
            if is_active {
                stdout.queue(SetBackgroundColor(Color::Rgb { r: 68, g: 71, b: 90 })).ok();
                stdout.queue(SetForegroundColor(Color::Rgb { r: 255, g: 255, b: 255 })).ok();
                stdout.queue(style::SetAttribute(Attribute::Bold)).ok();
            } else {
                stdout.queue(SetBackgroundColor(Color::Rgb { r: 40, g: 42, b: 54 })).ok();
                stdout.queue(SetForegroundColor(Color::Rgb { r: 140, g: 140, b: 140 })).ok();
                stdout.queue(style::SetAttribute(Attribute::NoBold)).ok();
            }

            // Truncate display name to fit
            let label = if col == 1 { "" } else { " " };
            let max_width = (phys_cols.saturating_sub(col + 1)) as usize;
            let display = if name.len() > max_width {
                format!("{}{}", label, &name[..max_width])
            } else {
                format!("{}{}", label, name)
            };

            if col + display.len() as u16 >= phys_cols {
                // Overflow — print ellipsis and stop
                stdout.queue(Print(format!("{}...", &display[..display.len().min(3)]))).ok();
                break;
            }

            stdout.queue(Print(&display)).ok();
            col += display.len() as u16;
        }

        // Clear remaining space
        stdout.queue(ResetColor).ok();
        stdout.queue(SetBackgroundColor(Color::Rgb { r: 40, g: 42, b: 54 })).ok();
        if (col as u16) < phys_cols {
            stdout.queue(MoveTo(col, 0)).ok();
            stdout.queue(crossterm::terminal::Clear(ClearType::UntilNewLine)).ok();
        }
        stdout.queue(ResetColor).ok();
        stdout.flush().ok();
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
                if display_all {
                    // Stay in display even when no commands exist — wait for
                    // new commands to be spawned via web UI, API, or F12.
                    tracing::info!("Display-all mode: entering monitor mode");
                    active_id = None;
                    exit_rx = None;
                } else if manager.list().is_empty() {
                    let _ = TerminalDisplay::clear();
                    break 'outer;
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
                // Fallback exit detection for the active command.
                if let Some(ref id) = active_id {
                    let gone = match manager.get(id) {
                        Some(h) => !h.is_alive(),
                        None => true,
                    };
                    if gone {
                        // Only dismiss the display if the DIRECT child
                        // (the CLI command) exited.  If a later-spawned
                        // command (via F12 / web UI) exited, switch to
                        // another running command instead.
                        let is_direct_child = direct_child_owned.as_deref() == Some(id);
                        if manager.list().is_empty() {
                            let _ = TerminalDisplay::clear();
                            break 'outer;
                        } else if is_direct_child && !display_all {
                            tracing::info!("Direct CLI command exited; dismissing display (commands remain)");
                            dismissed = true;
                            active_id = None;
                            exit_rx = None;
                        } else {
                            // Spawned command exited, or display_all mode —
                            // switch to another running command.
                            tracing::info!("Active command exited; switching to another command");
                            let commands = manager.list();
                            if let Some((new_id, new_name, _, new_pid, _)) = commands.first() {
                                active_id = Some(new_id.clone());
                                tracing::info!("Switched to {} (pid {})", new_name, new_pid);
                            } else {
                                active_id = None;
                            }
                        }
                    }
                }
                // For API-spawned commands (no direct child), check if all
                // commands have been removed.
                // When display_all is active, keep the display alive waiting
                // for new commands instead of breaking.
                if !display_all && exit_rx.is_none() && manager.list().is_empty() {
                    break;
                }

                // Handle escape sequence timeout — flush buffered bytes to command
                if let Some(deadline) = esc_deadline {
                    if tokio::time::Instant::now() >= deadline && !esc_buf.is_empty() {
                        // Timeout: forward buffered bytes to active command
                        let target_id = active_id.clone()
                            .or_else(|| manager.list().first().map(|(id, _, _, _, _)| id.clone()));
                        if let Some(ref tid) = target_id {
                            if let Some(handle) = manager.get(tid) {
                                let _ = handle.send_bytes(esc_buf.clone()).await;
                            }
                        }
                        esc_buf.clear();
                        esc_deadline = None;
                    }
                }

                // Render overlay or VTTY
                if showing_help {
                    // Re-render help overlay (e.g. after SIGWINCH)
                    render_help_overlay(&bindings, &mut stdout);
                } else if showing_log {
                    render_log_overlay(&manager, &log_entries, log_scroll_offset, &mut stdout);
                } else {
                    if show_tabs {
                        render_tab_bar(&manager, &active_id);
                    }
                    render_vtty(&manager, &active_id, if show_tabs { 1 } else { 0 }, display_all).await;
                }
            }

            // ── SIGWINCH — terminal resize ──
            _ = winch_rx.recv() => {
                if let Some((rows, cols)) = detect_terminal_size() {
                    // Subtract 1 row for the tab bar so the VTTY content
                    // fits the visible area.
                    let effective_rows = if show_tabs { rows.saturating_sub(1) } else { rows };
                    tracing::debug!(rows, cols, effective_rows, show_tabs, "SIGWINCH: terminal resized");
                    for entry in manager.list() {
                        let id = &entry.0;
                        if let Some(handle) = manager.get(id) {
                            if let Err(e) = handle.resize_pty(effective_rows, cols).await {
                                tracing::warn!(
                                    id = %id, effective_rows, cols, error = %e,
                                    "Failed to resize command on WINCH"
                                );
                            }
                        }
                    }
                }
            }

            // ── Keystroke forwarding with keybinding support ──
            // Read from /dev/tty via AsyncFd (no blocking thread — clean
            // exit).  We buffer escape sequences to match configurable
            // keybindings (e.g. Ctrl+Left/Right for command switching).
            // If no keybinding matches, all bytes are forwarded to the
            // active command.
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

                                    // Always-active hardcoded shortcuts
                                    if b == 0x1c {
                                        break 'outer;  // Ctrl+\ — quit display
                                    }
                                    if dismissed {
                                        if b == b'q' || b == 0x03 {
                                            break 'outer;  // q or Ctrl+C — shut down
                                        }
                                        continue;  // ignore other keys when dismissed
                                    }

                                    // ── Help overlay: any key dismisses ──
                                    if showing_help {
                                        showing_help = false;
                                        continue;
                                    }

                                    // If we're in the log overlay, handle navigation
                                    if showing_log {
                                        match b {
                                            // Same key toggles log off (check single-byte bindings)
                                            _ if esc_buf.is_empty() => {
                                                let single_buf = [b];
                                                let (action, _) = check_bindings(&single_buf, &bindings);
                                                if let Some(Action::ToggleLog) = action {
                                                    showing_log = false;
                                                    continue;
                                                }
                                                // 'q' or Esc closes log overlay
                                                if b == b'q' || b == 0x1b {
                                                    showing_log = false;
                                                    continue;
                                                }
                                                // Up/Down scroll the log (buffer ESC sequences)
                                                if b == 0x1b {
                                                    esc_buf.push(b);
                                                    esc_deadline = Some(tokio::time::Instant::now()
                                                        + tokio::time::Duration::from_millis(ESC_TIMEOUT_MS));
                                                    continue;
                                                }
                                                // Other keys in log view are ignored
                                                continue;
                                            }
                                            _ => continue,
                                        }
                                    }

                                    // ── Escape sequence buffering ──
                                    // Start buffering on ESC (0x1b), accumulate
                                    // subsequent bytes, then check for keybinding match.
                                    if b == 0x1b && esc_buf.is_empty() {
                                        esc_buf.push(b);
                                        esc_deadline = Some(tokio::time::Instant::now()
                                            + tokio::time::Duration::from_millis(ESC_TIMEOUT_MS));
                                        continue;
                                    }

                                    if !esc_buf.is_empty() {
                                        esc_buf.push(b);
                                        // Check if current buffer matches any keybinding
                                        let (action, partial) = check_bindings(&esc_buf, &bindings);
                                        if let Some(act) = action {
                                            // Clone the action before mutating esc_buf
                                            let act = act.clone();
                                            // Matched! Execute action and clear buffer.
                                            esc_buf.clear();
                                            esc_deadline = None;
                                            let effect = crate::interactive::execute_action(
                                                &act, showing_log, manager.list().len(), &bindings,
                                            );
                                            // spawn_command needs special handling: leave raw mode, read input
                                            if act == Action::SpawnCommand {
                                                // Leave raw mode, read command, re-enter
                                                let cmd_str = read_spawn_command();
                                                if !restore_raw_mode() {
                                                    break 'outer;
                                                }
                                                if let Some(cmd_str) = cmd_str {
                                                    let parts: Vec<&str> = cmd_str.split_whitespace().collect();
                                                    if !parts.is_empty() {
                                                        let cmd = parts[0].to_string();
                                                        let args = parts[1..].iter().map(|s| s.to_string()).collect();
                                                        match manager.spawn(cmd, args, None, None, std::collections::HashMap::new(), None, None).await {
                                                            Ok(id) => {
                                                                manager.logger().log("spawn_terminal", &format!("id={} cmd={}", id, cmd_str));
                                                                active_id = Some(id);
                                                            }
                                                            Err(e) => {
                                                                manager.logger().log("spawn_terminal_error", &format!("error={} cmd={}", e, cmd_str));
                                                            }
                                                        }
                                                    }
                                                }
                                                continue;
                                            }
                                            match effect {
                                                ActionEffect::None => continue,
                                                ActionEffect::NextCommand | ActionEffect::PrevCommand => {
                                                    let commands = manager.list();
                                                    if commands.len() <= 1 { continue; }
                                                    let current = active_id.clone()
                                                        .or_else(|| commands.first().map(|(id, _, _, _, _)| id.clone()));
                                                    if let Some(ref cur) = current {
                                                        let idx = commands.iter().position(|(id, _, _, _, _)| id == cur).unwrap_or(0);
                                                        let new_idx = if effect == ActionEffect::NextCommand {
                                                            (idx + 1) % commands.len()
                                                        } else {
                                                            idx.checked_sub(1).unwrap_or(commands.len() - 1)
                                                        };
                                                        let (new_id, new_name, _, new_pid, _) = &commands[new_idx];
                                                        active_id = Some(new_id.clone());
                                                        manager.logger().log("switch", &format!("id={} name={} pid={}", new_id, new_name, new_pid));
                                                        render_vtty(&manager, &active_id, if show_tabs { 1 } else { 0 }, display_all).await;
                                                    }
                                                }
                                                ActionEffect::ToggleLog(show) => {
                                                    showing_log = show;
                                                    log_scroll_offset = 0;
                                                }
                                                ActionEffect::ShowHelp => {
                                                    showing_help = true;
                                                    render_help_overlay(&bindings, &mut stdout);
                                                }
                                                ActionEffect::KillCommand => {
                                                    if let Some(ref id) = active_id {
                                                        manager.logger().log("kill_keybinding", &format!("id={}", id));
                                                        let _ = manager.kill(id, None).await;
                                                        active_id = None;
                                                    }
                                                }
                                                ActionEffect::TogglePause => {
                                                    if let Some(ref id) = active_id {
                                                        if let Some(handle) = manager.get(id) {
                                                            if handle.is_alive() {
                                                                let _ = manager.freeze(id);
                                                                manager.logger().log("freeze_keybinding", &format!("id={}", id));
                                                            } else {
                                                                let _ = manager.thaw(id);
                                                                manager.logger().log("thaw_keybinding", &format!("id={}", id));
                                                            }
                                                        }
                                                    }
                                                }
                                                ActionEffect::Quit => {
                                                    break 'outer;
                                                }
                                            }
                                            continue;
                                        } else if partial {
                                            // Might still match with more bytes — keep buffering
                                            continue;
                                        } else {
                                            // No match and no partial — forward all buffered bytes
                                            // to the active command, then clear the buffer.
                                            let target_id = active_id.clone()
                                                .or_else(|| manager.list().first().map(|(id, _, _, _, _)| id.clone()));
                                            if let Some(ref tid) = target_id {
                                                if let Some(handle) = manager.get(tid) {
                                                    let _ = handle.send_bytes(esc_buf.clone()).await;
                                                }
                                            }
                                            esc_buf.clear();
                                            esc_deadline = None;
                                            continue;
                                        }
                                    }

                                    // ── Single non-ESC byte: check single-byte keybindings ──
                                    let single_buf = [b];
                                    let (action, _) = check_bindings(&single_buf, &bindings);
                                    if let Some(act) = action {
                                        // spawn_command needs special handling
                                        if *act == Action::SpawnCommand {
                                            let cmd_str = read_spawn_command();
                                            if !restore_raw_mode() {
                                                break 'outer;
                                            }
                                            if let Some(cmd_str) = cmd_str {
                                                let parts: Vec<&str> = cmd_str.split_whitespace().collect();
                                                if !parts.is_empty() {
                                                    let cmd = parts[0].to_string();
                                                    let args = parts[1..].iter().map(|s| s.to_string()).collect();
                                                    match manager.spawn(cmd, args, None, None, std::collections::HashMap::new(), None, None).await {
                                                        Ok(id) => {
                                                            manager.logger().log("spawn_terminal", &format!("id={} cmd={}", id, cmd_str));
                                                            active_id = Some(id);
                                                        }
                                                        Err(e) => {
                                                            manager.logger().log("spawn_terminal_error", &format!("error={} cmd={}", e, cmd_str));
                                                        }
                                                    }
                                                }
                                            }
                                            continue;
                                        }
                                        let effect = crate::interactive::execute_action(
                                            act, showing_log, manager.list().len(), &bindings,
                                        );
                                        match effect {
                                            ActionEffect::None => continue,
                                            ActionEffect::NextCommand | ActionEffect::PrevCommand => {
                                                let commands = manager.list();
                                                if commands.len() <= 1 { continue; }
                                                let current = active_id.clone()
                                                    .or_else(|| commands.first().map(|(id, _, _, _, _)| id.clone()));
                                                if let Some(ref cur) = current {
                                                    let idx = commands.iter().position(|(id, _, _, _, _)| id == cur).unwrap_or(0);
                                                    let new_idx = if effect == ActionEffect::NextCommand {
                                                        (idx + 1) % commands.len()
                                                    } else {
                                                        idx.checked_sub(1).unwrap_or(commands.len() - 1)
                                                    };
                                                    let (new_id, new_name, _, new_pid, _) = &commands[new_idx];
                                                    active_id = Some(new_id.clone());
                                                    manager.logger().log("switch", &format!("id={} name={} pid={}", new_id, new_name, new_pid));
                                                    render_vtty(&manager, &active_id, if show_tabs { 1 } else { 0 }, display_all).await;
                                                }
                                            }
                                            ActionEffect::ToggleLog(show) => {
                                                showing_log = show;
                                                log_scroll_offset = 0;
                                            }
                                            ActionEffect::ShowHelp => {
                                                showing_help = true;
                                                render_help_overlay(&bindings, &mut stdout);
                                            }
                                            ActionEffect::KillCommand => {
                                                if let Some(ref id) = active_id {
                                                    manager.logger().log("kill_keybinding", &format!("id={}", id));
                                                    let _ = manager.kill(id, None).await;
                                                    active_id = None;
                                                }
                                            }
                                            ActionEffect::TogglePause => {
                                                if let Some(ref id) = active_id {
                                                    if let Some(handle) = manager.get(id) {
                                                        if handle.is_alive() {
                                                            let _ = manager.freeze(id);
                                                            manager.logger().log("freeze_keybinding", &format!("id={}", id));
                                                        } else {
                                                            let _ = manager.thaw(id);
                                                            manager.logger().log("thaw_keybinding", &format!("id={}", id));
                                                        }
                                                    }
                                                }
                                            }
                                            ActionEffect::Quit => {
                                                break 'outer;
                                            }
                                        }
                                        continue;
                                    }

                                    // ── Default: forward byte to active command ──
                                    let target_id = active_id.clone()
                                        .or_else(|| manager.list().first().map(|(id, _, _, _, _)| id.clone()));
                                    if let Some(ref tid) = target_id {
                                        if let Some(handle) = manager.get(tid) {
                                            let _ = handle.send_bytes(vec![b]).await;
                                        }
                                    }
                                }
                                Ok(Ok(_)) => {}  // ignore >1 byte
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

/// Render the command log as an overlay on top of the VTTY display.
/// Shows the most recent log entries, with the newest at the bottom.
pub fn render_log_overlay(
    _manager: &Arc<CommandManager>,
    log_entries: &Arc<std::sync::Mutex<Vec<String>>>,
    scroll_offset: usize,
    stdout: &mut std::io::Stdout,
) {
    use std::io::Write;
    let _ = crossterm::terminal::Clear(crossterm::terminal::ClearType::All);

    let entries = log_entries.lock().unwrap_or_else(|e| e.into_inner());
    let total = entries.len();

    // Show header
    let _ = write!(stdout, "\x1b[1;34m── Command Log ({} entries) ──\x1b[0m  Press q or Ctrl+L to close\r\n\r\n", total);

    // Get terminal height, leave room for header (2 lines) and footer (1 line)
    let (_, term_rows) = crossterm::terminal::size().unwrap_or((80, 24));
    let available_rows = (term_rows as usize).saturating_sub(3);

    // Calculate visible window
    let max_start = total.saturating_sub(available_rows);
    let start = if scroll_offset > max_start { max_start } else { scroll_offset };
    let end = (start + available_rows).min(total);

    for i in start..end {
        let _ = write!(stdout, "{}\r\n", &entries[i]);
    }

    // Footer
    let _ = write!(stdout, "\r\n\x1b[2mlines {}-{} of {}\x1b[0m", start + 1, end, total);
    let _ = stdout.flush();
}

/// Detect the terminal size using multiple methods, returning the most
/// reliable result.  Tries /dev/tty first (always the controlling terminal),
/// then stdout, then COLUMNS/LINES environment variables.
#[cfg(unix)]
pub fn detect_terminal_size() -> Option<(u16, u16)> {
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
pub fn detect_terminal_size() -> Option<(u16, u16)> {
    // crossterm returns (columns, rows); we need (rows, columns).
    crossterm::terminal::size().ok().map(|(cols, rows)| (rows, cols))
}

/// Wait for a direct child command to exit (headless, non-display mode).
///
/// Polls `kill(pid, 0)` at 500 ms intervals.  When the child is no longer
/// alive the function returns.
pub async fn wait_for_child(manager: &Arc<CommandManager>, id: &str) {
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
