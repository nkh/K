//! Interactive terminal display loop.
//!
//! This module contains the core display loop that renders VTTY buffers to
//! the local terminal, forwards keystrokes to child commands, and handles
//! terminal resize events (SIGWINCH). It is the primary interactive interface
//! for vrc's `--display` mode.
//!
//! The display loop runs entirely within the async runtime and uses crossterm
//! for terminal control, AsyncFd for non-blocking keystroke reading from
//! /dev/tty, and tokio channels for event coordination.
//!
//! The module is organized into sub-modules:
//! - [`mouse`] — Mouse event types, parsing, selection, clipboard (OSC 52)
//! - [`render`] — VTTY rendering: tab bar, split pane, search, context menu, watermark
//! - [`keybinding_dispatch`] — Keybinding action dispatch, spawn command, focus events

mod mouse;
mod render;
mod keybinding_dispatch;

use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};

use crate::interactive::Binding;
use crate::process::manager::CommandManager;

// Re-export sub-module items that are used externally
pub(crate) use mouse::{
    copy_selection_to_clipboard, render_selection_highlight, try_parse_mouse_event,
    MouseButton, MouseEventType,
};
pub(crate) use render::{
    find_search_matches, render_context_menu, render_exited_watermark,
    render_log_overlay, render_search_bar, render_search_highlights, render_split_pane,
    render_tab_bar, render_vtty,
};
pub(crate) use keybinding_dispatch::{
    dispatch_action, execute_context_menu_action, handle_spawn_command, send_focus_event,
    CommandLoopResult, SpawnCommandResult,
};

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
/// - `display_all == false` (default): the loop exits immediately when the
///   direct CLI command finishes (regardless of other running commands),
///   sending shutdown to terminate the process.  Use `--display-all` or
///   daemon mode to keep the display alive.
///
/// It renders the VTTY buffer to the local terminal using crossterm,
/// forwards all keystrokes to the active child command, and re-renders
/// on SIGWINCH (adapting the display to the new terminal size without
/// changing the fixed VTTY buffer dimensions).
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
#[allow(clippy::too_many_arguments)]
pub async fn run_display_loop(
    manager: &Arc<CommandManager>,
    direct_child_id: Option<&str>,
    refresh_ms: u64,
    display_all: bool,
    shutdown_tx: broadcast::Sender<()>,
    keybindings: &crate::config::schema::KeybindingsConfig,
    log_entries: &Arc<std::sync::Mutex<Vec<String>>>,
    show_tabs: bool,
    handle_sigwinch: bool,
) -> bool {
    // Architecture: tokio select! event loop with 4 async branches:
    //   1. Exit notification (watch channel) → transition or break
    //   2. Periodic tick → render, detect exits, handle overlays/mouse
    //   3. SIGWINCH (mpsc bridge) → wake the loop (display adapts on next tick)
    //   4. Keystroke (AsyncFd /dev/tty) → keybinding match or forward
    // Duplicated action dispatch (spawn, kill, switch, etc.) is extracted
    // into shared helper functions to avoid repetition.

    use crate::interactive::render_help_overlay;
    use crate::interactive::{check_bindings, resolve_keybindings, Action};
    use crate::vtty::display::TerminalDisplay;
    use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
    use crossterm::{cursor, ExecutableCommand};
    use std::io::Write;

    // Set up the alternate screen and raw mode.
    let mut stdout = std::io::stdout();
    if let Err(e) = terminal::enable_raw_mode() {
        tracing::warn!(error = %e, "Failed to enable raw mode");
        return true; // fatal setup error — shut down
    }
    let _ = stdout.execute(EnterAlternateScreen);
    let _ = stdout.execute(cursor::Hide);

    // ── Send focus gained event to commands with ?1004h enabled ──
    // When a command has enabled focus reporting, we send OSC 101 I
    // to indicate the terminal gained focus (display mode entered).
    send_focus_event(manager, true).await;

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
                return true; // fatal — shut down
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
                return true; // fatal — shut down
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

    // ── Visual bell state ──
    // When BEL (0x07) is received from any command, we flash the terminal
    // border briefly to provide visual feedback.  bell_until is the instant
    // after which the flash should stop.
    let mut bell_until: Option<tokio::time::Instant> = None;

    // ── Scrollback navigation state ──
    // Shift+Up/Down or Page Up/Down scroll the VTTY buffer backward into
    // scrollback history.  Any other key or VTTY output resets to 0.
    let mut scrollback_offset: usize = 0;

    // Build the keybinding lookup table using the interactive module.
    // Accepts both human-readable names ("ctrl+right") and raw escapes.
    let bindings: Vec<Binding> = resolve_keybindings(keybindings);

    // State for overlays
    let mut showing_log = false;
    let mut showing_help = false;
    let mut log_scroll_offset: usize = 0;

    // ── Search overlay state (#11) ──
    // Ctrl+F enters search mode. The user types a regex; matching cells
    // in the visible VTTY buffer are highlighted.  Enter jumps to next
    // match, Shift+Enter to previous, Escape closes search.
    let mut searching = false;
    let mut search_query: String = String::new();
    let mut search_regex: Option<regex::Regex> = None;
    let mut search_match_positions: Vec<(usize, usize, usize)> = Vec::new(); // (row, col, len)
    let mut search_current_match: usize = 0;
    // ── End search overlay state ──

    // ── Split-pane state (#14) ──
    // Ctrl+S toggles split-pane view showing two commands side-by-side.
    let mut split_mode = false;
    let mut split_right_id: Option<String> = None;
    // ── End split-pane state ──

    // ── Mouse selection / clipboard state (#15) ──
    // Enable mouse press/release/motion events for text selection.
    // On release, the selected text is copied to the clipboard via
    // the OSC 52 clipboard escape sequence (works in most modern terminals).
    let mut mouse_selection_start: Option<(u16, u16)> = None; // (row, col)
    let mut mouse_selection_end: Option<(u16, u16)> = None;
    let mut mouse_selecting = false;
    // Enable mouse tracking for selection (any-event tracking for wheel too)
    let _ = write!(stdout, "\x1b[?1003h"); // enable any-event mouse tracking (includes wheel)
    let _ = stdout.flush();
    // ── End mouse selection state ──

    // ── Tab position tracking (for mouse hit-testing) ──
    // Stores (id, start_col, end_col) for each visible tab.
    let mut tab_positions: Vec<(String, u16, u16)> = Vec::new();
    // ── End tab position tracking ──

    // ── Context menu state (right-click on tabs) ──
    let mut ctx_menu_visible = false;
    let mut ctx_menu_x: u16 = 0;
    let mut ctx_menu_y: u16 = 0;
    let mut ctx_menu_items: Vec<(&'static str, &'static str)> = Vec::new(); // (label, action_id)
    let mut ctx_menu_selected: usize = 0;
    let mut ctx_menu_target_id: Option<String> = None;
    // ── End context menu state ──

    // Set up SIGWINCH handler for terminal resize notification.
    // The handler simply wakes the select! loop — the actual display
    // re-renders on its next tick, detecting the new terminal size
    // automatically.  We do NOT resize VTTY buffers or PTYs on WINCH;
    // those dimensions are fixed at spawn time (CLI args or web UI)
    // and must only be changed programmatically (resize API, web UI
    // resize button, or vrc resize command).
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

    // When the last command exits, the display loop breaks and the process
    // shuts down — no waiting for user input.

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
                            let _ = terminal::disable_raw_mode();
                            let _ = stdout.execute(cursor::Show);
                            let _ = stdout.execute(LeaveAlternateScreen);
                            let _ = shutdown_tx.send(());
                            return true;
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
                            if !display_all {
                                let _ = terminal::disable_raw_mode();
                                let _ = stdout.execute(cursor::Show);
                                let _ = stdout.execute(LeaveAlternateScreen);
                                let _ = shutdown_tx.send(());
                                return true;
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
                        // Last command exited, not in daemon mode — exit immediately.
                        let _ = terminal::disable_raw_mode();
                        let _ = stdout.execute(cursor::Show);
                        let _ = stdout.execute(LeaveAlternateScreen);
                        let _ = shutdown_tx.send(());
                        return true;
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
                    // In display_all/tabs mode, transition to monitor mode only
                    // if retained commands remain.  If all commands have been
                    // removed (no retain_on_exit), exit immediately.
                    if direct_child_owned.is_some() && manager.list().is_empty() {
                        tracing::info!("Display-all mode: all commands exited, shutting down");
                        let _ = TerminalDisplay::clear();
                        break 'outer;
                    }
                    tracing::info!("Display-all mode: entering monitor mode");
                    active_id = None;
                    exit_rx = None;
                } else if manager.list().is_empty() {
                    let _ = TerminalDisplay::clear();
                    break 'outer;
                } else {
                    // Direct child exited but other commands remain (e.g.
                    // a restart spawned a replacement).  Switch to monitor
                    // mode instead of exiting so the server stays alive.
                    tracing::info!("Direct child exited but commands remain — entering monitor mode");
                    active_id = None;
                    exit_rx = None;
                }
            }

            // ── Periodic VTTY render ──
            _ = tick_rx.recv() => {
                // Fallback exit detection for the active command.
                if let Some(ref id) = active_id {
                    let gone = match manager.get(id) {
                        Some(h) => !h.is_alive(),
                        None => true,
                    };
                    if gone {
                        // Recover from stale alternate screen if the exited
                        // command left it active (common with vim, htop, less).
                        if let Some(ref cid) = active_id {
                            if let Some(handle) = manager.get(cid) {
                                handle.recover_alternate_screen().await;
                            }
                        }
                        // Only dismiss the display if the DIRECT child
                        // (the CLI command) exited.  If a later-spawned
                        // command (via F12 / web UI) exited, switch to
                        // another running command instead.
                        if manager.list().is_empty() {
                            // All commands gone — exit regardless of display_all
                            let _ = TerminalDisplay::clear();
                            break 'outer;
                        } else {
                            // Commands remain — switch to another running command.
                            // This covers: restart replacement, F12-spawned
                            // commands, and display_all mode.
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
                // commands have been removed.  When a direct child was
                // originally spawned (CLI), also exit if all commands are
                // gone — regardless of display_all mode.  Retained commands
                // (retain_on_exit=true) stay in the list, so an empty list
                // means nothing is left to display.
                if direct_child_owned.is_some() && exit_rx.is_none() && manager.list().is_empty() {
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
                    render_log_overlay(manager, log_entries, log_scroll_offset, &mut stdout);
                } else {
                    // Check for bell events from any command (visual feedback)
                    for entry in manager.list() {
                        if let Some(handle) = manager.get(&entry.0) {
                            let mut emu = handle.emulator.write().await;
                            if emu.drain_bell() {
                                bell_until = Some(tokio::time::Instant::now() + tokio::time::Duration::from_millis(150));
                            }
                        }
                    }
                    if show_tabs {
                        tab_positions = render_tab_bar(manager, &active_id);
                    }
                    if split_mode {
                        // Split-pane mode: render two VTTYs side-by-side
                        render_split_pane(manager, &active_id, &split_right_id, if show_tabs { 1 } else { 0 });
                    } else {
                        render_vtty(manager, &active_id, if show_tabs { 1 } else { 0 }, scrollback_offset, display_all).await;
                    }
                    // Render [EXITED] watermark if active command has exited
                    {
                        let target = active_id.clone()
                            .or_else(|| manager.list().first().map(|(id, _, _, _, _)| id.clone()));
                        if let Some(ref tid) = target {
                            if let Some(h) = manager.get(tid) {
                                if !h.is_alive() {
                                    let ec = h.exit_code.lock().ok().and_then(|c| *c);
                                    render_exited_watermark(if show_tabs { 1 } else { 0 }, ec);
                                }
                            }
                        }
                    }
                    if searching {
                        // Render search bar overlay
                        let is_error = search_regex.is_none() && !search_query.is_empty();
                        render_search_bar(&search_query, search_match_positions.len(), search_current_match, is_error);
                        // Highlight matches on top of VTTY
                        if !search_match_positions.is_empty() {
                            render_search_highlights(&search_match_positions, search_current_match, scrollback_offset, if show_tabs { 1 } else { 0 });
                        }
                    }
                    // Render visual bell flash overlay if active.
                    // Uses reverse-video on the entire visible area for 150ms.
                    if let Some(until) = bell_until {
                        if tokio::time::Instant::now() < until {
                            let (_, phys_rows) = crossterm::terminal::size().unwrap_or((80, 24));
                            let offset = if show_tabs { 1u16 } else { 0 };
                            let _ = write!(stdout, "\x1b[7m"); // reverse video on
                            for r in offset..phys_rows {
                                let _ = write!(stdout, "\x1b[{};1H", r + 1);
                                let _ = write!(stdout, "\x1b[2K"); // clear line (visible due to reverse)
                            }
                            let _ = write!(stdout, "\x1b[0m"); // reset
                            let _ = stdout.flush();
                        } else {
                            bell_until = None;
                        }
                    }
                    // Render selection highlight (#15)
                    if mouse_selecting {
                        if let (Some(start), Some(end)) = (mouse_selection_start, mouse_selection_end) {
                            render_selection_highlight(start, end, if show_tabs { 1 } else { 0 });
                        }
                    }
                    // Render context menu if visible
                    if ctx_menu_visible {
                        render_context_menu(ctx_menu_x, ctx_menu_y, &ctx_menu_items, ctx_menu_selected);
                    }
                }
            }

            // ── SIGWINCH — terminal resize ──
            // By default, VTTY dimensions are fixed at spawn time and
            // SIGWINCH only causes the display to re-render at the new
            // terminal size.  With --handle-sigwinch, SIGWINCH also resizes
            // all VTTY buffers and PTYs to match the terminal size.
            _ = winch_rx.recv() => {
                if handle_sigwinch {
                    if let Some((rows, cols)) = detect_terminal_size() {
                        let effective_rows = if show_tabs { rows.saturating_sub(1) } else { rows };
                        tracing::debug!(rows, cols, effective_rows, show_tabs, "SIGWINCH: resizing all VTTYs");
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
                } else {
                    tracing::debug!("SIGWINCH: display will adapt on next render tick");
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
                                    // ── Ctrl+F: toggle search mode ──
                                    if b == 0x06 && esc_buf.is_empty() && !showing_log && !showing_help {
                                        searching = !searching;
                                        if searching {
                                            search_query.clear();
                                            search_regex = None;
                                            search_match_positions.clear();
                                            search_current_match = 0;
                                        }
                                        continue;
                                    }
                                    // ── Ctrl+S: toggle split-pane (#14) ──
                                    if b == 0x13 && esc_buf.is_empty() && !showing_log && !showing_help && !searching {
                                        let commands = manager.list();
                                        if commands.len() >= 2 {
                                            split_mode = !split_mode;
                                            if split_mode {
                                                // Pick the next command for the right pane
                                                let current = active_id.clone()
                                                    .or_else(|| commands.first().map(|(id, _, _, _, _)| id.clone()));
                                                if let Some(ref cur) = current {
                                                    let idx = commands.iter().position(|(id, _, _, _, _)| id == cur).unwrap_or(0);
                                                    let next_idx = (idx + 1) % commands.len();
                                                    split_right_id = Some(commands[next_idx].0.clone());
                                                } else {
                                                    split_right_id = commands.get(1).map(|(id, _, _, _, _)| id.clone());
                                                }
                                            } else {
                                                split_right_id = None;
                                            }
                                        }
                                        continue;
                                    }
                                    // ── Help overlay: any key dismisses ──
                                    if showing_help {
                                        showing_help = false;
                                        continue;
                                    }

                                    // ── Context menu: Esc or Enter dismisses ──
                                    if ctx_menu_visible {
                                        match b {
                                            0x1b => {
                                                // Escape: close context menu
                                                ctx_menu_visible = false;
                                                ctx_menu_target_id = None;
                                                continue;
                                            }
                                            0x0d => {
                                                // Enter: execute selected context menu item
                                                if let Some(ref tid) = ctx_menu_target_id {
                                                    if let Some((_, action)) = ctx_menu_items.get(ctx_menu_selected) {
                                                        let new_active = execute_context_menu_action(
                                                            manager, action, tid, &active_id,
                                                        ).await;
                                                        active_id = new_active;
                                                        if active_id.as_deref() != Some(tid.as_str()) {
                                                            scrollback_offset = 0;
                                                        }
                                                    }
                                                }
                                                ctx_menu_visible = false;
                                                ctx_menu_target_id = None;
                                                continue;
                                            }
                                            // Arrow keys for navigation
                                            _ => continue,
                                        }
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

                                    // ── Search mode input handling ──
                                    if searching {
                                        match b {
                                            0x1b => {
                                                // Escape: close search
                                                searching = false;
                                                continue;
                                            }
                                            0x0d => {
                                                // Enter: next match
                                                if !search_match_positions.is_empty() {
                                                    search_current_match = (search_current_match + 1)
                                                        % search_match_positions.len();
                                                    // Scroll to make the match visible
                                                    let (row, _, _) = search_match_positions[search_current_match];
                                                    let (_, phys_rows) = crossterm::terminal::size().unwrap_or((80, 24));
                                                    let visible_start = scrollback_offset;
                                                    let visible_end = scrollback_offset + (phys_rows as usize);
                                                    if row < visible_start || row >= visible_end {
                                                        scrollback_offset = row.saturating_sub(phys_rows as usize / 3);
                                                    }
                                                }
                                                continue;
                                            }
                                            0x7f | 0x08 => {
                                                // Backspace: delete last char
                                                search_query.pop();
                                                // Re-compile regex
                                                search_regex = regex::Regex::new(&search_query).ok();
                                                search_match_positions = if let Some(ref re) = search_regex {
                                                    find_search_matches(manager, &active_id, re)
                                                } else {
                                                    Vec::new()
                                                };
                                                search_current_match = 0;
                                                continue;
                                            }
                                            _ => {
                                                // Regular printable char: append to query
                                                search_query.push(b as char);
                                                // Re-compile and search
                                                search_regex = regex::Regex::new(&search_query).ok();
                                                search_match_positions = if let Some(ref re) = search_regex {
                                                    find_search_matches(manager, &active_id, re)
                                                } else {
                                                    Vec::new()
                                                };
                                                search_current_match = 0;
                                                continue;
                                            }
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
                                        // ── Check for mouse events first ──
                                        if let Some(me) = try_parse_mouse_event(&esc_buf) {
                                            // Save raw bytes before clearing — needed
                                            // for forwarding middle/right clicks to child
                                            let raw_mouse_bytes = esc_buf.clone();
                                            esc_buf.clear();
                                            esc_deadline = None;

                                            // Dismiss context menu on any mouse event outside it
                                            if ctx_menu_visible && me.event_type == MouseEventType::Press {
                                                ctx_menu_visible = false;
                                                ctx_menu_target_id = None;
                                            }

                                            // Handle context menu navigation
                                            if ctx_menu_visible {
                                                match me.button {
                                                    MouseButton::WheelUp => {
                                                        ctx_menu_selected = ctx_menu_selected.saturating_sub(1);
                                                        continue;
                                                    }
                                                    MouseButton::WheelDown => {
                                                        if ctx_menu_selected + 1 < ctx_menu_items.len() {
                                                            ctx_menu_selected += 1;
                                                        }
                                                        continue;
                                                    }
                                                    MouseButton::Left if me.event_type == MouseEventType::Press => {
                                                        // Execute selected context menu action
                                                        if let Some(ref tid) = ctx_menu_target_id {
                                                            if let Some((_, action)) = ctx_menu_items.get(ctx_menu_selected) {
                                                                let new_active = execute_context_menu_action(
                                                                    manager, action, tid, &active_id,
                                                                ).await;
                                                                active_id = new_active;
                                                                if active_id.as_deref() != Some(tid.as_str()) {
                                                                    scrollback_offset = 0;
                                                                }
                                                            }
                                                        }
                                                        ctx_menu_visible = false;
                                                        ctx_menu_target_id = None;
                                                        continue;
                                                    }
                                                    _ => continue,
                                                }
                                            }

                                            let tab_off = if show_tabs { 1u16 } else { 0 };

                                            match me.button {
                                                // ── Left click in tabs area: switch tab ──
                                                MouseButton::Left if me.event_type == MouseEventType::Press && show_tabs && me.y == 0 => {
                                                    // Hit-test tabs
                                                    for (id, start, end) in &tab_positions {
                                                        if me.x >= *start && me.x < *end {
                                                            active_id = Some(id.clone());
                                                            scrollback_offset = 0;
                                                            mouse_selecting = false;
                                                            manager.logger().log("tab_click", &format!("id={}", id));
                                                            break;
                                                        }
                                                    }
                                                    continue;
                                                }
                                                // ── Left click outside tabs: selection or forward to terminal ──
                                                MouseButton::Left => {
                                                    match me.event_type {
                                                        MouseEventType::Press => {
                                                            if mouse_selecting {
                                                                // Drag: extend selection
                                                                mouse_selection_end = Some((me.y.saturating_sub(tab_off), me.x));
                                                            } else if me.y >= tab_off && me.y < crossterm::terminal::size().unwrap_or((80, 24)).1.saturating_sub(1) {
                                                                mouse_selection_start = Some((me.y.saturating_sub(tab_off), me.x));
                                                                mouse_selection_end = Some((me.y.saturating_sub(tab_off), me.x));
                                                                mouse_selecting = true;
                                                            }
                                                        }
                                                        MouseEventType::Release => {
                                                            if mouse_selecting {
                                                                mouse_selecting = false;
                                                                if let (Some(start), Some(end)) = (mouse_selection_start, mouse_selection_end) {
                                                                    let tab_off = if show_tabs { 1u16 } else { 0 };
                                                                    copy_selection_to_clipboard(manager, &active_id, start, end, tab_off);
                                                                }
                                                                mouse_selection_start = None;
                                                                mouse_selection_end = None;
                                                            }
                                                        }
                                                        MouseEventType::Motion => {
                                                            if mouse_selecting {
                                                                mouse_selection_end = Some((me.y.saturating_sub(tab_off), me.x));
                                                            }
                                                        }
                                                    }
                                                    continue;
                                                }
                                                // ── Right click in tabs area: context menu ──
                                                MouseButton::Right if me.event_type == MouseEventType::Press && show_tabs && me.y == 0 => {
                                                    // Find which tab was right-clicked
                                                    for (id, start, end) in &tab_positions {
                                                        if me.x >= *start && me.x < *end {
                                                            let is_exited = manager.get(id).map(|h| !h.is_alive()).unwrap_or(false);
                                                            let mut items: Vec<(&'static str, &'static str)> = vec![
                                                                ("Kill", "kill"),
                                                                ("Copy ID", "copy_id"),
                                                            ];
                                                            if is_exited {
                                                                items.insert(0, ("Restart", "restart"));
                                                                items.insert(1, ("Purge", "purge"));
                                                            }
                                                            ctx_menu_items = items;
                                                            ctx_menu_selected = 0;
                                                            ctx_menu_x = me.x;
                                                            ctx_menu_y = me.y + 1;
                                                            ctx_menu_target_id = Some(id.clone());
                                                            ctx_menu_visible = true;
                                                            mouse_selecting = false;
                                                            break;
                                                        }
                                                    }
                                                    continue;
                                                }
                                                // ── Mouse wheel ──
                                                MouseButton::WheelUp | MouseButton::WheelDown => {
                                                    if show_tabs && me.y == 0 {
                                                        // Wheel in tab bar: cycle through tabs
                                                        let commands = manager.list();
                                                        if commands.len() > 1 {
                                                            let current = active_id.clone()
                                                                .or_else(|| commands.first().map(|(id, _, _, _, _)| id.clone()));
                                                            if let Some(ref cur) = current {
                                                                let idx = commands.iter().position(|(id, _, _, _, _)| id == cur).unwrap_or(0);
                                                                let new_idx = if me.button == MouseButton::WheelUp {
                                                                    idx.wrapping_sub(1)
                                                                } else {
                                                                    (idx + 1) % commands.len()
                                                                };
                                                                let (new_id, _, _, _, _) = &commands[new_idx];
                                                                active_id = Some(new_id.clone());
                                                                scrollback_offset = 0;
                                                            }
                                                        }
                                                    } else {
                                                        // Wheel outside tabs: scrollback navigation
                                                        if me.button == MouseButton::WheelUp {
                                                            scrollback_offset = scrollback_offset.saturating_add(3);
                                                        } else {
                                                            scrollback_offset = scrollback_offset.saturating_sub(3);
                                                        }
                                                    }
                                                    continue;
                                                }
                                                // ── Middle/right click: forward to terminal ──
                                                _ => {
                                                    // Forward the raw escape sequence to the child
                                                    // (applications that use mouse events, e.g. htop, vim)
                                                    if !mouse_selecting && me.y >= tab_off {
                                                        let target_id = active_id.clone()
                                                            .or_else(|| manager.list().first().map(|(id, _, _, _, _)| id.clone()));
                                                        if let Some(ref tid) = target_id {
                                                            if let Some(handle) = manager.get(tid) {
                                                                let _ = handle.send_bytes(raw_mouse_bytes).await;
                                                            }
                                                        }
                                                    }
                                                    continue;
                                                }
                                            }
                                        }
                                        // ── Partial mouse event detection ──
                                        // When mouse tracking is active, the terminal sends
                                        // SGR (\x1b[<...) or legacy (\x1b[M...) mouse sequences.
                                        // These are NOT prefixes of any keybinding, so
                                        // check_bindings returns (None, false) for partial
                                        // mouse sequences like \x1b[<.  Without this guard,
                                        // partial mouse bytes are forwarded to the child
                                        // command, causing display artifacts and blinking.
                                        if esc_buf.starts_with(b"\x1b[<") || esc_buf.starts_with(b"\x1b[M") {
                                            continue; // keep buffering — mouse event incomplete
                                        }
                                        // Check if current buffer matches any keybinding
                                        let (action, partial) = check_bindings(&esc_buf, &bindings);
                                        if let Some(act) = action {
                                            // Clone the action before mutating esc_buf
                                            let act = act.clone();
                                            // Matched! Execute action and clear buffer.
                                            esc_buf.clear();
                                            esc_deadline = None;
                                            // spawn_command needs special handling: leave raw mode, read input
                                            if act == Action::SpawnCommand {
                                                match handle_spawn_command(manager).await {
                                                    SpawnCommandResult::ShouldBreak => break 'outer,
                                                    SpawnCommandResult::Spawned(id) => {
                                                        active_id = Some(id);
                                                        scrollback_offset = 0;
                                                    }
                                                    SpawnCommandResult::NoOp => {}
                                                }
                                                continue;
                                            }
                                            match dispatch_action(
                                                manager, &act, &mut active_id, &mut log_scroll_offset,
                                                &mut showing_log, &mut showing_help, &mut scrollback_offset,
                                            ).await {
                                                CommandLoopResult::Break => break 'outer,
                                                CommandLoopResult::Continue => continue,
                                                CommandLoopResult::RenderAndContinue => {
                                                    if showing_help {
                                                        render_help_overlay(&bindings, &mut stdout);
                                                    } else {
                                                        render_vtty(manager, &active_id, if show_tabs { 1 } else { 0 }, scrollback_offset, display_all).await;
                                                    }
                                                }
                                            }
                                            continue;
                                        } else if partial {
                                            // Might still match with more bytes — keep buffering
                                            continue;
                                        } else {
                                            // No match and no partial — check for scroll keys
                                            // before forwarding to the command.
                                            let scroll_delta = match esc_buf.as_slice() {
                                                // Page Up: ESC [ 5 ~ — scroll up by half a page
                                                [0x1b, b'[', b'5', b'~'] => {
                                                    let (_, rows) = crossterm::terminal::size().unwrap_or((80, 24));
                                                    (rows as usize / 2) as isize
                                                }
                                                // Page Down: ESC [ 6 ~ — scroll down
                                                [0x1b, b'[', b'6', b'~'] => {
                                                    let (_, rows) = crossterm::terminal::size().unwrap_or((80, 24));
                                                    -((rows as usize / 2) as isize)
                                                }
                                                // Shift+Up: ESC [ 1 ; 2 A — scroll up 1 line
                                                [0x1b, b'[', b'1', b';', b'2', b'A'] => 1,
                                                // Shift+Down: ESC [ 1 ; 2 B — scroll down 1 line
                                                [0x1b, b'[', b'1', b';', b'2', b'B'] => -1,
                                                _ => 0,
                                            };
                                            if scroll_delta != 0 {
                                                scrollback_offset = scrollback_offset
                                                    .saturating_add(scroll_delta as usize);
                                                esc_buf.clear();
                                                esc_deadline = None;
                                                continue;
                                            }
                                            // Forward to the active command
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
                                            match handle_spawn_command(manager).await {
                                                SpawnCommandResult::ShouldBreak => break 'outer,
                                                SpawnCommandResult::Spawned(id) => {
                                                    active_id = Some(id);
                                                    scrollback_offset = 0;
                                                }
                                                SpawnCommandResult::NoOp => {}
                                            }
                                            continue;
                                        }
                                        match dispatch_action(
                                            manager, act, &mut active_id, &mut log_scroll_offset,
                                            &mut showing_log, &mut showing_help, &mut scrollback_offset,
                                        ).await {
                                            CommandLoopResult::Break => break 'outer,
                                            CommandLoopResult::Continue => continue,
                                            CommandLoopResult::RenderAndContinue => {
                                                if showing_help {
                                                    render_help_overlay(&bindings, &mut stdout);
                                                } else {
                                                    render_vtty(manager, &active_id, if show_tabs { 1 } else { 0 }, scrollback_offset, display_all).await;
                                                }
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
    // Disable mouse tracking
    let _ = write!(stdout, "\x1b[?1003l"); // disable any-event mouse tracking
    let _ = stdout.flush();
    let _ = stdout.execute(cursor::Show);
    let _ = stdout.execute(LeaveAlternateScreen);
    let _ = stdout.flush();
    let _ = terminal::disable_raw_mode();
    let _ = stdout.flush();

    // If we broke out of the loop, always trigger shutdown.
    let _ = shutdown_tx.send(());
    true
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
                    rows,
                    cols,
                    method = "/dev/tty",
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
                rows,
                cols,
                method = "crossterm",
                "Terminal size from crossterm (stdout)"
            );
            return Some((rows, cols));
        }
    }

    // Method 3: COLUMNS / LINES environment variables.
    if let (Ok(cols_str), Ok(rows_str)) = (std::env::var("COLUMNS"), std::env::var("LINES")) {
        if let (Ok(cols), Ok(rows)) = (cols_str.parse::<u16>(), rows_str.parse::<u16>()) {
            if rows > 0 && cols > 0 {
                tracing::debug!(
                    rows,
                    cols,
                    method = "env",
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
    crossterm::terminal::size()
        .ok()
        .map(|(cols, rows)| (rows, cols))
}

/// Wait for a child command to exit, or for a shutdown signal.
/// In headless mode this blocks the main loop until either:
///   1. The child process exits (natural termination), or
///   2. A SIGTERM/SIGINT is received (external stop)
///
/// When a shutdown signal arrives, all managed commands are killed
/// so the instance exits cleanly.
pub async fn wait_for_child(
    manager: &Arc<CommandManager>,
    id: &str,
    mut shutdown_rx: tokio::sync::broadcast::Receiver<()>,
) {
    let mut interval = tokio::time::interval(std::time::Duration::from_millis(500));
    let mut original_alive = true;
    loop {
        tokio::select! {
            _ = interval.tick() => {
                if original_alive {
                    if let Some(handle) = manager.get(&id.to_string()) {
                        let pid = handle.pid as i32;
                        let alive = unsafe { libc::kill(pid, 0) == 0 };
                        if !alive {
                            tracing::info!(id, "Direct child exited");
                            return;
                        }
                    } else {
                        // Original command removed (e.g. killed by a restart).
                        // Only exit if no replacement commands are running.
                        if manager.list().is_empty() {
                            tracing::info!(id, "Direct child removed and no commands remain — exiting");
                            return;
                        }
                        tracing::info!(id, "Direct child removed but commands remain (likely restart) — watching remaining");
                        original_alive = false;
                    }
                } else {
                    // Original is gone; watch the remaining command list.
                    if manager.list().is_empty() {
                        tracing::info!("All commands exited — shutting down");
                        return;
                    }
                }
            }
            _ = shutdown_rx.recv() => {
                tracing::info!(id, "Shutdown signal received in headless mode");
                // Kill the child so vrc can exit cleanly
                let id_string = id.to_string();
                let _ = manager.kill(&id_string, None).await;
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::mouse::base64_encode;
    use super::render::build_cell_sgr;

    #[test]
    fn test_mouse_event_sgr_press() {
        // SGR left press at (10, 20): ESC [ < 0 ; 10 ; 20 M
        let buf = b"\x1b[<0;10;20M";
        let event = try_parse_mouse_event(buf).unwrap();
        assert_eq!(event.button, MouseButton::Left);
        assert_eq!(event.event_type, MouseEventType::Press);
        assert_eq!(event.x, 10);
        assert_eq!(event.y, 20);
    }

    #[test]
    fn test_mouse_event_sgr_release() {
        // SGR left release at (10, 20): ESC [ < 0 ; 10 ; 20 m
        let buf = b"\x1b[<0;10;20m";
        let event = try_parse_mouse_event(buf).unwrap();
        assert_eq!(event.button, MouseButton::Left);
        assert_eq!(event.event_type, MouseEventType::Release);
    }

    #[test]
    fn test_mouse_event_wheel_up() {
        // SGR wheel up: ESC [ < 64 ; 50 ; 10 M
        let buf = b"\x1b[<64;50;10M";
        let event = try_parse_mouse_event(buf).unwrap();
        assert_eq!(event.button, MouseButton::WheelUp);
        assert_eq!(event.event_type, MouseEventType::Press);
    }

    #[test]
    fn test_base64_encode() {
        assert_eq!(base64_encode(""), "");
        assert_eq!(base64_encode("f"), "Zg==");
        assert_eq!(base64_encode("fo"), "Zm8=");
        assert_eq!(base64_encode("foo"), "Zm9v");
        assert_eq!(base64_encode("Hello, World!"), "SGVsbG8sIFdvcmxkIQ==");
    }

    #[test]
    fn test_build_cell_sgr_default() {
        let cell = crate::vtty::cell::Cell {
            ch: ' ',
            fg: [204, 204, 204],
            bg: [0, 0, 0],
            bold: false,
            italic: false,
            underline: false,
            blink: false,
            reverse: false,
            invisible: false,
            strikethrough: false,
            width: 1,
        };
        let sgr = build_cell_sgr(&cell);
        assert_eq!(sgr, "\x1b[0m");
    }

    #[test]
    fn test_build_cell_sgr_bold() {
        let cell = crate::vtty::cell::Cell {
            ch: 'A',
            fg: [255, 0, 0],
            bg: [0, 0, 0],
            bold: true,
            italic: false,
            underline: false,
            blink: false,
            reverse: false,
            invisible: false,
            strikethrough: false,
            width: 1,
        };
        let sgr = build_cell_sgr(&cell);
        assert!(sgr.contains("\x1b[1m")); // bold
        assert!(sgr.contains("\x1b[38;2;255;0;0m")); // red fg
    }
}
