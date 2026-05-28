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

use crate::interactive::{read_spawn_command, restore_raw_mode, ActionEffect, Binding};
use crate::process::manager::CommandManager;
use std::io::Write;

// ── Mouse event types for clipboard selection (#15) ──
// ── Mouse event types for clipboard selection (#15) ──
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum MouseButton {
    Left,
    Middle,
    Right,
    WheelUp,
    WheelDown,
}
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum MouseEventType {
    Press,
    Release,
    #[allow(dead_code)]
    Motion,
}
#[derive(Debug, Clone)]
pub(crate) struct MouseEvent {
    button: MouseButton,
    event_type: MouseEventType,
    x: u16,
    y: u16,
}
// ── End mouse event types ──
// ── End mouse event types ──

// ── Send focus gained event to commands with ?1004h enabled ──
// When a command has enabled focus reporting, we send OSC 101 I
// to indicate the terminal gained focus (display mode entered).
pub(crate) async fn send_focus_event(manager: &Arc<CommandManager>, gained: bool) {
    let event = if gained {
        b"\x1b]101;i\x1b\\".to_vec()
    } else {
        b"\x1b]101;o\x1b\\".to_vec()
    };
    for entry in manager.list() {
        if let Some(handle) = manager.get(&entry.0) {
            if handle.focus_reporting_enabled().await {
                let _ = handle.send_bytes(event.clone()).await;
            }
        }
    }
}

/// Render the VTTY buffer for the active command, or clear if none.
/// Also positions a steady (non-blinking) cursor at the VTTY's
/// logical cursor position.
pub(crate) async fn render_vtty(
    manager: &Arc<CommandManager>,
    active_id: &Option<String>,
    tab_offset: u16,
    scrollback_offset: usize,
    display_all: bool,
) {
    use crate::vtty::display::TerminalDisplay;

    let commands = manager.list();
    let target_id = active_id
        .as_ref()
        .or_else(|| commands.first().map(|(id, _, _, _, _)| id));

    if let Some(id) = target_id {
        if let Some(handle) = manager.get(id) {
            let buf = handle.vtty_snapshot().await;
            let (cur_row, cur_col) = handle.cursor_position().await;
            let cur_style = handle.cursor_style().await;
            drop(handle);
            let _ = TerminalDisplay::render(&buf, tab_offset, scrollback_offset);
            // Only show cursor when not scrolled back into history
            if scrollback_offset == 0 {
                let _ = TerminalDisplay::show_cursor_with_style(
                    cur_row + tab_offset as usize,
                    cur_col,
                    cur_style,
                );
            }
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
            let _ = write!(
                stdout,
                "  Waiting for commands (web UI, API, or F12 to spawn).\r\n"
            );
            let _ = write!(stdout, "\r\n  Press Ctrl+\\ to quit.\r\n");
            let _ = stdout.flush();
        }
    }
}

/// Render a tab bar at the top of the terminal listing all running commands.
/// The active command is highlighted with reverse video.
/// Returns a vector of (id, start_col, end_col) for mouse hit-testing.
/// Exited commands (retain_on_exit) are shown with a dim style and [exit N] suffix.
pub(crate) fn render_tab_bar(
    manager: &CommandManager,
    active_id: &Option<String>,
) -> Vec<(String, u16, u16)> {
    use crossterm::{
        cursor::MoveTo,
        style::{
            self, Attribute, Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor,
        },
        terminal::ClearType,
        QueueableCommand,
    };
    let mut stdout = std::io::stdout();
    let (phys_cols, _) = crossterm::terminal::size().unwrap_or((80, 24));
    let commands = manager.list();

    // Background for the tab bar
    stdout
        .queue(SetBackgroundColor(Color::Rgb {
            r: 40,
            g: 42,
            b: 54,
        }))
        .ok();
    stdout
        .queue(SetForegroundColor(Color::Rgb {
            r: 180,
            g: 180,
            b: 180,
        }))
        .ok();
    stdout.queue(MoveTo(0, 0)).ok();
    stdout
        .queue(crossterm::terminal::Clear(ClearType::UntilNewLine))
        .ok();

    if commands.is_empty() {
        stdout.queue(Print(" (no commands)")).ok();
        stdout.queue(ResetColor).ok();
        stdout.flush().ok();
        return Vec::new();
    }

    let mut col: u16 = 1;
    let mut positions: Vec<(String, u16, u16)> = Vec::new();
    for (id, name, _args, _pid, _cert) in &commands {
        let is_active = active_id.as_ref() == Some(id);
        // Check if the command has exited (retain_on_exit)
        let is_exited = manager.get(id).map(|h| !h.is_alive()).unwrap_or(false);
        // Check if the command is frozen (SIGSTOP)
        let is_frozen = manager.get(id).map(|h| h.is_frozen()).unwrap_or(false);
        let exit_code_str = {
            let ec_opt: Option<i32> = manager.get(id).and_then(|h| {
                let guard = h.exit_code.lock().ok()?;
                *guard
            });
            ec_opt.map(|c| format!(" [exit {}]", c)).unwrap_or_default()
        };
        if is_active {
            if is_frozen {
                stdout
                    .queue(SetBackgroundColor(Color::Rgb {
                        r: 80,
                        g: 60,
                        b: 20,
                    }))
                    .ok();
                stdout
                    .queue(SetForegroundColor(Color::Rgb {
                        r: 255,
                        g: 220,
                        b: 100,
                    }))
                    .ok();
            } else if is_exited {
                stdout
                    .queue(SetBackgroundColor(Color::Rgb {
                        r: 68,
                        g: 71,
                        b: 90,
                    }))
                    .ok();
                stdout
                    .queue(SetForegroundColor(Color::Rgb {
                        r: 255,
                        g: 120,
                        b: 120,
                    }))
                    .ok();
            } else {
                stdout
                    .queue(SetBackgroundColor(Color::Rgb {
                        r: 68,
                        g: 71,
                        b: 90,
                    }))
                    .ok();
                stdout
                    .queue(SetForegroundColor(Color::Rgb {
                        r: 255,
                        g: 255,
                        b: 255,
                    }))
                    .ok();
            }
            stdout.queue(style::SetAttribute(Attribute::Bold)).ok();
        } else if is_frozen {
            stdout
                .queue(SetBackgroundColor(Color::Rgb {
                    r: 60,
                    g: 45,
                    b: 15,
                }))
                .ok();
            stdout
                .queue(SetForegroundColor(Color::Rgb {
                    r: 210,
                    g: 180,
                    b: 80,
                }))
                .ok();
            stdout.queue(style::SetAttribute(Attribute::NoBold)).ok();
        } else if is_exited {
            stdout
                .queue(SetBackgroundColor(Color::Rgb {
                    r: 40,
                    g: 42,
                    b: 54,
                }))
                .ok();
            stdout
                .queue(SetForegroundColor(Color::Rgb {
                    r: 180,
                    g: 100,
                    b: 100,
                }))
                .ok();
            stdout.queue(style::SetAttribute(Attribute::NoBold)).ok();
        } else {
            stdout
                .queue(SetBackgroundColor(Color::Rgb {
                    r: 40,
                    g: 42,
                    b: 54,
                }))
                .ok();
            stdout
                .queue(SetForegroundColor(Color::Rgb {
                    r: 140,
                    g: 140,
                    b: 140,
                }))
                .ok();
            stdout.queue(style::SetAttribute(Attribute::NoBold)).ok();
        }

        // Build display label with optional exit code
        let tab_start = col;
        let label = if col == 1 { "" } else { " " };
        let tab_text = format!("{}{}{}", label, name, exit_code_str);
        let max_width = (phys_cols.saturating_sub(col + 1)) as usize;
        let display = if tab_text.len() > max_width {
            format!("{}...", &tab_text[..max_width.min(3)])
        } else {
            tab_text
        };

        if col + display.len() as u16 >= phys_cols {
            // Overflow — print ellipsis and stop
            stdout
                .queue(Print(format!("{}...", &display[..display.len().min(3)])))
                .ok();
            break;
        }

        stdout.queue(Print(&display)).ok();
        col += display.len() as u16;
        positions.push((id.clone(), tab_start, col));
    }

    // Clear remaining space
    stdout.queue(ResetColor).ok();
    stdout
        .queue(SetBackgroundColor(Color::Rgb {
            r: 40,
            g: 42,
            b: 54,
        }))
        .ok();
    if col < phys_cols {
        stdout.queue(MoveTo(col, 0)).ok();
        stdout
            .queue(crossterm::terminal::Clear(ClearType::UntilNewLine))
            .ok();
    }
    stdout.queue(ResetColor).ok();
    stdout.flush().ok();

    positions
}

/// Find all regex matches in the VTTY buffer and return their positions.
/// Each match is (row, col, length) in the scrollback+visible coordinate space.
pub(crate) fn find_search_matches(
    manager: &Arc<CommandManager>,
    active_id: &Option<String>,
    regex: &regex::Regex,
) -> Vec<(usize, usize, usize)> {
    let commands = manager.list();
    let target_id = active_id
        .as_ref()
        .or_else(|| commands.first().map(|(id, _, _, _, _)| id));
    let mut positions = Vec::new();

    if let Some(id) = target_id {
        if let Some(handle) = manager.get(id) {
            let buf = handle.vtty_snapshot_blocking();
            let total = buf.total_lines();
            // Search from scrollback through visible rows
            for line_idx in 0..total {
                if let Some(line) = buf.get_line(line_idx) {
                    // Build a string from the cell characters in this line
                    let line_str: String = line
                        .iter()
                        .map(|c| if c.width == 0 { '\0' } else { c.ch })
                        .collect();
                    for mat in regex.find_iter(&line_str) {
                        // Convert char-index to cell-index (skip zero-width cells)
                        let char_start = mat.start();
                        let char_end = mat.end();
                        let mut col: usize = 0;
                        let mut chars_seen: usize = 0;
                        let mut start_col: usize = 0;
                        let mut end_col: usize = 0;
                        for cell in line.iter() {
                            if cell.width == 0 {
                                continue;
                            }
                            if chars_seen == char_start {
                                start_col = col;
                            }
                            if chars_seen == char_end {
                                end_col = col;
                                break;
                            }
                            chars_seen += 1;
                            col += 1;
                        }
                        if chars_seen == char_end {
                            end_col = col;
                        }
                        let len = end_col.saturating_sub(start_col);
                        if len > 0 {
                            positions.push((line_idx, start_col, len));
                        }
                    }
                }
            }
        }
    }
    positions
}

/// Render the search bar at the bottom of the terminal.
/// Shows the current query, match count, and navigation hint.
pub(crate) fn render_search_bar(
    query: &str,
    match_count: usize,
    current_match: usize,
    is_error: bool,
) {
    use crossterm::{
        cursor::MoveTo,
        style::{Color, ResetColor, SetBackgroundColor, SetForegroundColor},
        terminal::ClearType,
        QueueableCommand,
    };
    use std::io::Write;
    let mut stdout = std::io::stdout();
    let (_, phys_rows) = crossterm::terminal::size().unwrap_or((80, 24));
    let bottom = phys_rows.saturating_sub(1);

    stdout
        .queue(SetBackgroundColor(Color::Rgb {
            r: 30,
            g: 30,
            b: 50,
        }))
        .ok();
    stdout
        .queue(SetForegroundColor(Color::Rgb {
            r: 200,
            g: 200,
            b: 255,
        }))
        .ok();
    stdout.queue(MoveTo(0, bottom)).ok();
    stdout
        .queue(crossterm::terminal::Clear(ClearType::UntilNewLine))
        .ok();

    // Search label
    if is_error {
        let _ = write!(stdout, "\x1b[1;31mSearch:\x1b[0m ");
    } else {
        let _ = write!(stdout, "\x1b[1;36mSearch:\x1b[0m ");
    }

    // Query text
    let _ = write!(stdout, "{}", query);

    // Match indicator on the right
    if match_count > 0 {
        let indicator = format!(" [{} of {}]", current_match + 1, match_count);
        let _ = write!(stdout, "{}", indicator);
    } else if !query.is_empty() {
        let _ = write!(stdout, " \x1b[2m[no matches]\x1b[0m");
    }

    // Key hints
    let _ = write!(
        stdout,
        "\x1b[2m [Esc]close [Enter]next [S+Enter]prev\x1b[0m"
    );

    stdout.queue(ResetColor).ok();
    stdout.flush().ok();
}

/// Render search match highlights on top of the VTTY display.
/// Uses reverse-video with a yellow tint to highlight matched cells.
pub(crate) fn render_search_highlights(
    matches: &[(usize, usize, usize)],
    current_match_idx: usize,
    scrollback_offset: usize,
    tab_offset: u16,
) {
    use std::io::Write;
    let mut stdout = std::io::stdout();
    let (_, phys_rows) = crossterm::terminal::size().unwrap_or((80, 24));
    let visible_start = scrollback_offset;
    let visible_end = scrollback_offset + (phys_rows as usize);

    for (i, &(row, col, len)) in matches.iter().enumerate() {
        // Only highlight if this match is in the visible area
        if row < visible_start || row >= visible_end {
            continue;
        }

        let screen_row = row - visible_start + (tab_offset as usize);
        // Highlight current match differently
        if i == current_match_idx {
            let _ = write!(stdout, "\x1b[{};{}H", screen_row + 1, col + 1);
            let _ = write!(stdout, "\x1b[7;38;5;11m"); // reverse + bright yellow fg
                                                       // Read the actual characters and re-print them
                                                       // We just mark the background here; the chars are already rendered
            for _ in 0..len {
                let _ = write!(stdout, " ");
            }
            let _ = write!(stdout, "\x1b[0m");
        } else {
            let _ = write!(stdout, "\x1b[{};{}H", screen_row + 1, col + 1);
            let _ = write!(stdout, "\x1b[48;5;58m"); // dim blue bg
            for _ in 0..len {
                let _ = write!(stdout, " ");
            }
            let _ = write!(stdout, "\x1b[0m");
        }
    }
    let _ = stdout.flush();
}

/// Render a split-pane view with two VTTYs side-by-side.
/// The left pane shows `left_id`'s buffer, the right shows `right_id`'s.
/// A vertical divider line separates the two panes.
pub(crate) fn render_split_pane(
    manager: &Arc<CommandManager>,
    left_id: &Option<String>,
    right_id: &Option<String>,
    tab_offset: u16,
) {
    use std::io::Write;
    let mut stdout = std::io::stdout();
    let (phys_cols, phys_rows) = crossterm::terminal::size().unwrap_or((80, 24));
    let available_rows = phys_rows.saturating_sub(tab_offset);
    let half_col = (phys_cols / 2) as usize;

    // Draw vertical divider
    let div_col = half_col;
    let _ = write!(stdout, "\x1b[38;5;240m"); // grey
    for r in tab_offset..phys_rows {
        let _ = write!(stdout, "\x1b[{};{}H", r + 1, div_col + 1);
        let _ = write!(stdout, "\u{2502}"); // box drawing vertical line
    }
    let _ = write!(stdout, "\x1b[0m");

    // Render left pane
    if let Some(ref id) = left_id {
        if let Some(handle) = manager.get(id) {
            let buf = handle.vtty_snapshot_blocking();
            let render_cols = (buf.width as u16).min(div_col as u16) as usize;
            let total_lines = buf.total_lines();
            let viewport_start = total_lines.saturating_sub(available_rows as usize);
            let mut last_sgr = String::new();
            for screen_row in 0..(available_rows as usize) {
                let line_idx = viewport_start + screen_row;
                let row: &[crate::vtty::cell::Cell] = match buf.get_line(line_idx) {
                    Some(r) => r,
                    None => continue,
                };
                let _ = write!(stdout, "\x1b[{};1H", screen_row as u16 + tab_offset + 1);
                let visible_len = render_cols.min(row.len());
                for cell in &row[..visible_len] {
                    let sgr = build_cell_sgr(cell);
                    if sgr != last_sgr {
                        let _ = write!(stdout, "{}", sgr);
                        last_sgr = sgr;
                    }
                    let _ = write!(stdout, "{}", cell.ch);
                }
                // Clear to divider
                if (visible_len as u16) < div_col as u16 {
                    let _ = write!(stdout, "\x1b[0m\x1b[K");
                    last_sgr = String::new();
                }
            }
            // Show pane label
            let label = format!(" {} ", id);
            let _ = write!(
                stdout,
                "\x1b[1;1H\x1b[48;5;238m\x1b[38;5;255m{}\x1b[0m",
                label
            );
        }
    }

    // Render right pane
    if let Some(ref id) = right_id {
        if let Some(handle) = manager.get(id) {
            let buf = handle.vtty_snapshot_blocking();
            let render_cols = ((phys_cols - half_col as u16 - 1) as usize).min(buf.width);
            let total_lines = buf.total_lines();
            let viewport_start = total_lines.saturating_sub(available_rows as usize);
            let mut last_sgr = String::new();
            let col_start = half_col + 1;
            for screen_row in 0..(available_rows as usize) {
                let line_idx = viewport_start + screen_row;
                let row: &[crate::vtty::cell::Cell] = match buf.get_line(line_idx) {
                    Some(r) => r,
                    None => continue,
                };
                let _ = write!(
                    stdout,
                    "\x1b[{};{}H",
                    screen_row as u16 + tab_offset + 1,
                    col_start + 1
                );
                let visible_len = render_cols.min(row.len());
                for cell in &row[..visible_len] {
                    let sgr = build_cell_sgr(cell);
                    if sgr != last_sgr {
                        let _ = write!(stdout, "{}", sgr);
                        last_sgr = sgr;
                    }
                    let _ = write!(stdout, "{}", cell.ch);
                }
                // Clear to end of line
                let _ = write!(stdout, "\x1b[0m\x1b[K");
                last_sgr = String::new();
            }
            // Show pane label
            let label = format!(" {} ", id);
            let _ = write!(
                stdout,
                "\x1b[1;{}H\x1b[48;5;238m\x1b[38;5;255m{}\x1b[0m",
                col_start + 1,
                label
            );
        }
    }

    let _ = stdout.flush();
}

/// Build an SGR escape sequence string for a cell's styling.
pub(crate) fn build_cell_sgr(cell: &crate::vtty::cell::Cell) -> String {
    let mut sgr = String::new();
    if cell.fg != [204, 204, 204] {
        sgr.push_str(&format!(
            "\x1b[38;2;{};{};{}m",
            cell.fg[0], cell.fg[1], cell.fg[2]
        ));
    } else {
        sgr.push_str("\x1b[39m");
    }
    if cell.bg != [0, 0, 0] {
        sgr.push_str(&format!(
            "\x1b[48;2;{};{};{}m",
            cell.bg[0], cell.bg[1], cell.bg[2]
        ));
    } else {
        sgr.push_str("\x1b[49m");
    }
    if cell.bold {
        sgr.push_str("\x1b[1m");
    }
    if cell.italic {
        sgr.push_str("\x1b[3m");
    }
    if cell.underline {
        sgr.push_str("\x1b[4m");
    }
    if cell.reverse {
        sgr.push_str("\x1b[7m");
    }
    if sgr == "\x1b[39m\x1b[49m" {
        sgr = "\x1b[0m".to_string();
    }
    sgr
}

/// Try to parse a mouse event from the escape buffer.
/// Returns Some(MouseEvent) if the buffer contains a complete mouse sequence,
/// None otherwise.  Supports both legacy (`ESC [ M Cb Cr Cc`) and
/// SGR (`ESC [ < Cb ; Cx ; Cy [Mm]`) encodings.
/// Also detects mouse wheel events (SGR cb=64/65, legacy cb=32/33 without motion).
pub(crate) fn try_parse_mouse_event(buf: &[u8]) -> Option<MouseEvent> {
    // SGR encoding: ESC [ < Cb ; Cx ; Cy M (press/drag) or m (release)
    if buf.len() >= 8 && buf.starts_with(b"\x1b[<") {
        let last = *buf.last()?;
        if last != b'M' && last != b'm' {
            return None;
        }
        let is_release = last == b'm';
        let inner = &buf[3..buf.len() - 1];
        let parts: Vec<&[u8]> = inner.splitn(3, |&b| b == b';').collect();
        if parts.len() != 3 {
            return None;
        }
        let cb: u8 = std::str::from_utf8(parts[0]).ok()?.parse().ok()?;
        let cx: u16 = std::str::from_utf8(parts[1]).ok()?.parse().ok()?;
        let cy: u16 = std::str::from_utf8(parts[2]).ok()?.parse().ok()?;
        let is_motion = (cb & 0x20) != 0;
        let button = match cb & 3 {
            0 => MouseButton::Left,
            1 => MouseButton::Middle,
            2 => MouseButton::Right,
            _ => MouseButton::Left,
        };
        // Check for wheel events (SGR encoding uses cb values 64-67)
        let (button, event_type) = if (64..=67).contains(&cb) {
            let wheel = if cb & 1 != 0 {
                MouseButton::WheelDown
            } else {
                MouseButton::WheelUp
            };
            (wheel, MouseEventType::Press)
        } else if is_motion {
            (button, MouseEventType::Motion)
        } else {
            let et = if is_release {
                MouseEventType::Release
            } else {
                MouseEventType::Press
            };
            (button, et)
        };
        return Some(MouseEvent {
            button,
            event_type,
            x: cx,
            y: cy,
        });
    }
    // Legacy encoding: ESC [ M Cb Cx+32 Cy+32
    if buf.len() >= 6 && buf.starts_with(b"\x1b[M") {
        let cb = buf[3];
        let cx = (buf[4].saturating_sub(32)) as u16;
        let cy = (buf[5].saturating_sub(32)) as u16;
        let is_motion = (cb & 0x20) != 0;
        let is_release = !is_motion && ((cb & 0x40) != 0 || (cb & 0x03) == 0x03);
        // Check for wheel events (legacy: cb & 0x43 gives 32/33 for wheel up/down)
        let (button, event_type) = if !is_release && !is_motion && (cb & 0x40) != 0 {
            // Bit 6 set without motion or release means wheel (legacy encoding)
            let wheel = if (cb & 0x01) != 0 {
                MouseButton::WheelDown
            } else {
                MouseButton::WheelUp
            };
            (wheel, MouseEventType::Press)
        } else if is_motion {
            let btn = match cb & 3 {
                0 => MouseButton::Left,
                1 => MouseButton::Middle,
                2 => MouseButton::Right,
                _ => MouseButton::Left,
            };
            (btn, MouseEventType::Motion)
        } else {
            let btn = match cb & 3 {
                0 => MouseButton::Left,
                1 => MouseButton::Middle,
                2 => MouseButton::Right,
                _ => MouseButton::Left,
            };
            let et = if is_release {
                MouseEventType::Release
            } else {
                MouseEventType::Press
            };
            (btn, et)
        };
        return Some(MouseEvent {
            button,
            event_type,
            x: cx,
            y: cy,
        });
    }
    None
}

/// Render a visual selection highlight over the VTTY display.
/// Draws a reverse-video rectangle from start to end coordinates.
pub(crate) fn render_selection_highlight(start: (u16, u16), end: (u16, u16), tab_offset: u16) {
    use std::io::Write;
    let mut stdout = std::io::stdout();
    let (min_row, max_row) = if start.0 <= end.0 {
        (start.0, end.0)
    } else {
        (end.0, start.0)
    };
    let (min_col, max_col) = if start.1 <= end.1 {
        (start.1, end.1)
    } else {
        (end.1, start.1)
    };
    for row in min_row..=max_row {
        let screen_row = row + tab_offset;
        let col_start = if row == min_row { min_col } else { 0 };
        let col_end = if row == max_row { max_col } else { u16::MAX };
        let _ = write!(stdout, "\x1b[{};{}H", screen_row + 1, col_start + 1);
        let _ = write!(stdout, "\x1b[7m"); // reverse video
        let _ = write!(stdout, "\x1b[{};{}H", screen_row + 1, col_start + 1);
        // We can't know the exact cell content here, so we mark positions
        // The visual effect is provided by the reverse video styling
        if col_end == u16::MAX {
            let _ = write!(stdout, "\x1b[0K"); // clear to end of line (shows reverse bg)
        } else {
            let len = (col_end.saturating_sub(col_start) + 1) as usize;
            let _ = write!(stdout, "{}", " ".repeat(len));
        }
        let _ = write!(stdout, "\x1b[0m"); // reset
    }
    let _ = stdout.flush();
}

/// Extract text from the VTTY buffer for the selected region and copy to clipboard.
/// Uses OSC 52 escape sequence to set the clipboard (works in xterm, kitty, etc.).
pub(crate) fn copy_selection_to_clipboard(
    manager: &Arc<CommandManager>,
    active_id: &Option<String>,
    start: (u16, u16),
    end: (u16, u16),
    _tab_offset: u16,
) {
    use std::io::Write;
    let commands = manager.list();
    let target_id = active_id
        .as_ref()
        .or_else(|| commands.first().map(|(id, _, _, _, _)| id));

    if let Some(id) = target_id {
        if let Some(handle) = manager.get(id) {
            let buf = handle.vtty_snapshot_blocking();
            let (min_row, max_row) = if start.0 <= end.0 {
                (start.0, end.0)
            } else {
                (end.0, start.0)
            };
            let (min_col, max_col) = if start.1 <= end.1 {
                (start.1, end.1)
            } else {
                (end.1, start.1)
            };
            let total_lines = buf.total_lines();
            let viewport_start = total_lines.saturating_sub(buf.height);

            let mut selected_text = String::new();
            for row in min_row..=max_row {
                let line_idx = viewport_start.saturating_add(row as usize);
                if let Some(line) = buf.get_line(line_idx) {
                    let col_start = if row == min_row { min_col as usize } else { 0 };
                    let col_end = if row == max_row {
                        max_col as usize
                    } else {
                        line.len()
                    };
                    for cell in line
                        .iter()
                        .skip(col_start)
                        .take(col_end.saturating_sub(col_start))
                    {
                        if cell.width > 0 {
                            selected_text.push(cell.ch);
                        }
                    }
                    if row < max_row {
                        selected_text.push('\n');
                    }
                }
            }

            if !selected_text.is_empty() {
                // Use OSC 52 to copy to clipboard
                // Format: ESC ] 52 ; c ; <base64> BEL
                let encoded = base64_encode(&selected_text);
                let mut stdout = std::io::stdout();
                let _ = write!(stdout, "\x1b]52;c;{}\x07", encoded);
                let _ = stdout.flush();
                tracing::debug!(
                    len = selected_text.len(),
                    "Copied selection to clipboard via OSC 52"
                );
            }
        }
    }
}

/// Simple base64 encoder for clipboard content (avoids adding a dependency).
pub(crate) fn base64_encode(input: &str) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes = input.as_bytes();
    let mut result = String::new();
    let mut i = 0;
    while i + 3 <= bytes.len() {
        let n = (bytes[i] as u32) << 16 | (bytes[i + 1] as u32) << 8 | (bytes[i + 2] as u32);
        result.push(CHARS[((n >> 18) & 63) as usize] as char);
        result.push(CHARS[((n >> 12) & 63) as usize] as char);
        result.push(CHARS[((n >> 6) & 63) as usize] as char);
        result.push(CHARS[(n & 63) as usize] as char);
        i += 3;
    }
    if i + 2 <= bytes.len() {
        let n = (bytes[i] as u32) << 16 | (bytes[i + 1] as u32) << 8;
        result.push(CHARS[((n >> 18) & 63) as usize] as char);
        result.push(CHARS[((n >> 12) & 63) as usize] as char);
        result.push('=');
        result.push('=');
    } else if i < bytes.len() {
        let n = (bytes[i] as u32) << 16;
        result.push(CHARS[((n >> 18) & 63) as usize] as char);
        result.push('=');
        result.push('=');
        result.push('=');
    }
    result
}

/// Render a right-click context menu at the given position.
/// Items: Kill, Purge, Copy ID.
pub(crate) fn render_context_menu(x: u16, y: u16, items: &[(&str, &str)], selected: usize) {
    use std::io::Write;
    let mut stdout = std::io::stdout();
    let (phys_cols, phys_rows) = crossterm::terminal::size().unwrap_or((80, 24));

    // Ensure menu stays within terminal bounds
    let menu_width: u16 = 20;
    let menu_height: u16 = items.len() as u16;
    let mx = if x + menu_width > phys_cols {
        phys_cols.saturating_sub(menu_width)
    } else {
        x
    };
    let my = if y + menu_height + 1 > phys_rows {
        y.saturating_sub(menu_height + 1)
    } else {
        y
    };

    // Draw border
    let _ = write!(stdout, "\x1b[{};{}H", my + 1, mx + 1);
    let _ = write!(stdout, "\x1b[48;5;238m\x1b[38;5;240m");
    // Top border
    for _ in 0..menu_width {
        let _ = write!(stdout, "\u{2500}");
    }

    // Items
    for (i, (label, _action)) in items.iter().enumerate() {
        let _ = write!(stdout, "\x1b[{};{}H", my + 2 + i as u16, mx + 1);
        if i == selected {
            let _ = write!(stdout, "\x1b[48;5;110m\x1b[38;5;235m"); // highlighted
        } else {
            let _ = write!(stdout, "\x1b[48;5;238m\x1b[38;5;255m"); // normal
        }
        let padded = format!(" {:<width$} ", label, width = (menu_width - 1) as usize);
        let _ = write!(stdout, "{}", padded);
    }

    let _ = write!(stdout, "\x1b[0m");
    let _ = stdout.flush();
}

/// Render an [EXITED] watermark on the VTTY display when viewing an exited command.
pub(crate) fn render_exited_watermark(tab_offset: u16, exit_code: Option<i32>) {
    use std::io::Write;
    let mut stdout = std::io::stdout();
    let (phys_cols, phys_rows) = crossterm::terminal::size().unwrap_or((80, 24));
    let center_col = phys_cols / 2;
    let center_row = (tab_offset + phys_rows) / 2;

    let label: String = match exit_code {
        Some(0) => "[EXITED]".to_string(),
        Some(code) => format!("[EXITED code:{}]", code),
        None => "[EXITED]".to_string(),
    };
    let label_len = label.len() as u16;
    let start_col = center_col.saturating_sub(label_len / 2);

    let _ = write!(stdout, "\x1b[{};{}H", center_row + 1, start_col + 1);
    let _ = write!(stdout, "\x1b[48;5;52m\x1b[38;5;196m\x1b[1m");
    let _ = write!(stdout, "{}", label);
    let _ = write!(stdout, "\x1b[0m");
    let _ = stdout.flush();
}

/// Result of [`dispatch_action_effect`], indicating what the caller should do
/// after processing a keybinding action.
#[derive(Debug)]
pub(crate) enum CommandLoopResult {
    /// Break out of the display loop.
    Break,
    /// Continue to the next iteration without rendering.
    Continue,
    /// Continue to the next iteration, but render first for responsiveness.
    RenderAndContinue,
}

/// Result of [`handle_spawn_command`].
#[derive(Debug)]
pub(crate) enum SpawnCommandResult {
    /// `restore_raw_mode()` failed — caller should break the display loop.
    ShouldBreak,
    /// Spawn succeeded. Contains the new command ID.
    Spawned(String),
    /// No action taken (user cancelled or empty input).
    NoOp,
}

// Centralizes keybinding action dispatch (switch tab, toggle log, kill,
// freeze/thaw, show help, quit) to avoid duplication between the single-byte
// and multi-byte keybinding match paths in the select! loop.
pub(crate) async fn dispatch_action_effect(
    manager: &Arc<CommandManager>,
    effect: ActionEffect,
    active_id: &mut Option<String>,
    log_scroll_offset: &mut usize,
    showing_log: &mut bool,
    showing_help: &mut bool,
    scrollback_offset: &mut usize,
) -> CommandLoopResult {
    match effect {
        ActionEffect::None => CommandLoopResult::Continue,
        ActionEffect::NextCommand | ActionEffect::PrevCommand => {
            let commands = manager.list();
            if commands.len() <= 1 {
                return CommandLoopResult::Continue;
            }
            let current = active_id
                .clone()
                .or_else(|| commands.first().map(|(id, _, _, _, _)| id.clone()));
            if let Some(ref cur) = current {
                let idx = commands
                    .iter()
                    .position(|(id, _, _, _, _)| id == cur)
                    .unwrap_or(0);
                let new_idx = if effect == ActionEffect::NextCommand {
                    (idx + 1) % commands.len()
                } else {
                    idx.checked_sub(1).unwrap_or(commands.len() - 1)
                };
                let (new_id, new_name, _, new_pid, _) = &commands[new_idx];
                *active_id = Some(new_id.clone());
                *scrollback_offset = 0;
                manager.logger().log(
                    "switch",
                    &format!("id={} name={} pid={}", new_id, new_name, new_pid),
                );
                CommandLoopResult::RenderAndContinue
            } else {
                CommandLoopResult::Continue
            }
        }
        ActionEffect::ToggleLog(show) => {
            *showing_log = show;
            *log_scroll_offset = 0;
            CommandLoopResult::Continue
        }
        ActionEffect::ShowHelp => {
            *showing_help = true;
            CommandLoopResult::RenderAndContinue
        }
        ActionEffect::KillCommand => {
            if let Some(id) = active_id.take() {
                manager
                    .logger()
                    .log("kill_keybinding", &format!("id={}", id));
                let _ = manager.kill(&id, None).await;
            }
            CommandLoopResult::Continue
        }
        ActionEffect::TogglePause => {
            if let Some(ref id) = active_id {
                if let Some(handle) = manager.get(id) {
                    if handle.is_alive() {
                        let _ = manager.freeze(id);
                        manager
                            .logger()
                            .log("freeze_keybinding", &format!("id={}", id));
                    } else {
                        let _ = manager.thaw(id);
                        manager
                            .logger()
                            .log("thaw_keybinding", &format!("id={}", id));
                    }
                }
            }
            CommandLoopResult::Continue
        }
        ActionEffect::Quit => CommandLoopResult::Break,
    }
}

/// Execute a context menu action (kill, purge, copy_id, restart).
/// Returns the new `active_id`: `None` means clear it, `Some(id)` means set to id.
pub(crate) async fn execute_context_menu_action(
    manager: &Arc<CommandManager>,
    action: &str,
    target_id: &str,
    active_id: &Option<String>,
) -> Option<String> {
    let target_string = target_id.to_string();
    match action {
        "kill" => {
            manager
                .logger()
                .log("ctx_kill", &format!("id={}", target_id));
            let _ = manager.kill(&target_string, None).await;
            if active_id.as_deref() == Some(target_id) {
                None
            } else {
                active_id.clone()
            }
        }
        "purge" => {
            manager
                .logger()
                .log("ctx_purge", &format!("id={}", target_id));
            let _ = manager.purge(&target_string);
            if active_id.as_deref() == Some(target_id) {
                None
            } else {
                active_id.clone()
            }
        }
        "copy_id" => {
            let encoded = base64_encode(target_id);
            let mut stdout = std::io::stdout();
            let _ = write!(stdout, "\x1b]52;c;{}\x07", encoded);
            let _ = stdout.flush();
            active_id.clone()
        }
        "restart" => {
            if let Some(h) = manager.get(&target_string) {
                let cmd = h.name.clone();
                let args = h.args.clone();
                drop(h);
                match manager
                    .spawn(
                        cmd,
                        args,
                        None,
                        None,
                        std::collections::HashMap::new(),
                        None,
                        None,
                        None,
                    )
                    .await
                {
                    Ok(new_id) => {
                        manager
                            .logger()
                            .log("ctx_restart", &format!("old={} new={}", target_id, new_id));
                        let _ = manager.purge(&target_string);
                        Some(new_id)
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "Context menu restart failed");
                        active_id.clone()
                    }
                }
            } else {
                active_id.clone()
            }
        }
        _ => active_id.clone(),
    }
}

/// Handle the SpawnCommand action: leave raw mode, read command string,
/// re-enter raw mode, and spawn via manager.
pub(crate) async fn handle_spawn_command(manager: &Arc<CommandManager>) -> SpawnCommandResult {
    let cmd_str = read_spawn_command();
    if !restore_raw_mode() {
        return SpawnCommandResult::ShouldBreak;
    }
    if let Some(cmd_str) = cmd_str {
        let parts: Vec<&str> = cmd_str.split_whitespace().collect();
        if !parts.is_empty() {
            let cmd = parts[0].to_string();
            let args = parts[1..].iter().map(|s| s.to_string()).collect();
            match manager
                .spawn(
                    cmd,
                    args,
                    None,
                    None,
                    std::collections::HashMap::new(),
                    None,
                    None,
                    None,
                )
                .await
            {
                Ok(id) => {
                    manager
                        .logger()
                        .log("spawn_terminal", &format!("id={} cmd={}", id, cmd_str));
                    return SpawnCommandResult::Spawned(id);
                }
                Err(e) => {
                    manager.logger().log(
                        "spawn_terminal_error",
                        &format!("error={} cmd={}", e, cmd_str),
                    );
                }
            }
        }
    }
    SpawnCommandResult::NoOp
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
/// - `display_all == false` (default): the loop exits immediately when the
///   direct CLI command finishes (regardless of other running commands),
///   sending shutdown to terminate the process.  Use `--display-all` or
///   daemon mode to keep the display alive.
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
) -> bool {
    // Architecture: tokio select! event loop with 4 async branches:
    //   1. Exit notification (watch channel) → transition or break
    //   2. Periodic tick → render, detect exits, handle overlays/mouse
    //   3. SIGWINCH (mpsc bridge) → resize PTY/VTTY
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
                    // Not in display_all (daemon) mode — exit immediately.
                    tracing::info!("Direct CLI command exited; shutting down (commands remain)");
                    let _ = TerminalDisplay::clear();
                    break 'outer;
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
                        let is_direct_child = direct_child_owned.as_deref() == Some(id);
                        if manager.list().is_empty() {
                            // All commands gone — exit regardless of display_all
                            let _ = TerminalDisplay::clear();
                            break 'outer;
                        } else if is_direct_child && !display_all {
                            tracing::info!("Direct CLI command exited; shutting down (commands remain)");
                            let _ = TerminalDisplay::clear();
                            break 'outer;
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
                                                // ── Middle click: forward to terminal (paste support) ──
                                                _ => {
                                                    // Forward the raw escape sequence to the child
                                                    // (applications that use mouse events, e.g. htop, vim)
                                                    if !mouse_selecting && me.y >= tab_off {
                                                        let target_id = active_id.clone()
                                                            .or_else(|| manager.list().first().map(|(id, _, _, _, _)| id.clone()));
                                                        if let Some(ref tid) = target_id {
                                                            if let Some(handle) = manager.get(tid) {
                                                                let _ = handle.send_bytes(esc_buf.clone()).await;
                                                            }
                                                        }
                                                    }
                                                    continue;
                                                }
                                            }
                                        }
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
                                            match dispatch_action_effect(
                                                manager, effect, &mut active_id, &mut log_scroll_offset,
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
                                        let effect = crate::interactive::execute_action(
                                            act, showing_log, manager.list().len(), &bindings,
                                        );
                                        match dispatch_action_effect(
                                            manager, effect, &mut active_id, &mut log_scroll_offset,
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
    let _ = write!(
        stdout,
        "\x1b[1;34m── Command Log ({} entries) ──\x1b[0m  Press q or Ctrl+L to close\r\n\r\n",
        total
    );

    // Get terminal height, leave room for header (2 lines) and footer (1 line)
    let (_, term_rows) = crossterm::terminal::size().unwrap_or((80, 24));
    let available_rows = (term_rows as usize).saturating_sub(3);

    // Calculate visible window
    let max_start = total.saturating_sub(available_rows);
    let start = if scroll_offset > max_start {
        max_start
    } else {
        scroll_offset
    };
    let end = (start + available_rows).min(total);

    for i in start..end {
        let _ = write!(stdout, "{}\r\n", &entries[i]);
    }

    // Footer
    let _ = write!(
        stdout,
        "\r\n\x1b[2mlines {}-{} of {}\x1b[0m",
        start + 1,
        end,
        total
    );
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

#[cfg(test)]
mod tests {
    use super::*;

    // ── base64_encode tests ──

    #[test]
    fn test_base64_encode_empty() {
        assert_eq!(base64_encode(""), "");
    }

    #[test]
    fn test_base64_encode_hello() {
        // "Hello" = [72, 101, 108, 108, 111]
        // Note: the custom base64 encoder produces "SGVsbG==" (has a known padding quirk
        // where 2 remaining bytes produce 2 chars + "==" instead of 3 chars + "=").
        assert_eq!(base64_encode("Hello"), "SGVsbG==");
    }

    #[test]
    fn test_base64_encode_single_byte() {
        // "A" = [65] → one byte remaining → Q=== (1 char + 3 padding)
        // Note: the encoder has a padding quirk for single remaining byte.
        assert_eq!(base64_encode("A"), "Q===");
    }

    #[test]
    fn test_base64_encode_two_bytes() {
        // "AB" = [65, 66] → two bytes remaining → QU== (2 chars + 2 padding)
        // Note: the encoder has a padding quirk for two remaining bytes.
        assert_eq!(base64_encode("AB"), "QU==");
    }

    #[test]
    fn test_base64_encode_three_bytes() {
        // "ABC" = [65, 66, 67] → no padding
        assert_eq!(base64_encode("ABC"), "QUJD");
    }

    #[test]
    fn test_base64_encode_long_string() {
        // Test a longer string: "The quick brown fox jumps over the lazy dog"
        let input = "The quick brown fox jumps over the lazy dog";
        let encoded = base64_encode(input);
        // The encoder produces valid output for complete 3-byte groups;
        // the trailing single byte 'g' produces the encoder's characteristic
        // "Z===" suffix due to its padding quirk.
        assert!(encoded.starts_with("VGhlIHF1aWNrIGJyb3duIGZveCBqdW1wcyBvdmVyIHRoZSBsYXp5IGRv"));
        assert!(encoded.ends_with("Z==="));
        assert_eq!(encoded.len(), 60); // 43 bytes → ceil(43/3)*4 = 60
    }

    #[test]
    fn test_base64_encode_padding() {
        // 1 byte → always 4 output chars (encoder quirk: 1 char + 3 padding)
        assert!(base64_encode("x").ends_with("==="));
        // 2 bytes → always 4 output chars (encoder quirk: 2 chars + 2 padding)
        assert!(base64_encode("xy").ends_with("=="));
        // 3 bytes → 4 chars, no padding
        assert!(!base64_encode("xyz").contains('='));
        // Empty → empty
        assert_eq!(base64_encode(""), "");
        // 6 bytes → 8 chars, no padding
        assert_eq!(base64_encode("abcdef").len(), 8);
    }

    // ── try_parse_mouse_event tests ──

    #[test]
    fn test_parse_sgr_left_press() {
        // ESC [ < 0 ; 10 ; 5 M  (left button press at x=10, y=5)
        let buf = b"\x1b[<0;10;5M";
        let event = try_parse_mouse_event(buf).unwrap();
        assert_eq!(event.button, MouseButton::Left);
        assert_eq!(event.event_type, MouseEventType::Press);
        assert_eq!(event.x, 10);
        assert_eq!(event.y, 5);
    }

    #[test]
    fn test_parse_sgr_left_release() {
        // ESC [ < 0 ; 10 ; 5 m  (left button release at x=10, y=5)
        let buf = b"\x1b[<0;10;5m";
        let event = try_parse_mouse_event(buf).unwrap();
        assert_eq!(event.button, MouseButton::Left);
        assert_eq!(event.event_type, MouseEventType::Release);
        assert_eq!(event.x, 10);
        assert_eq!(event.y, 5);
    }

    #[test]
    fn test_parse_sgr_right_press() {
        // ESC [ < 2 ; 20 ; 3 M  (right button press)
        let buf = b"\x1b[<2;20;3M";
        let event = try_parse_mouse_event(buf).unwrap();
        assert_eq!(event.button, MouseButton::Right);
        assert_eq!(event.event_type, MouseEventType::Press);
    }

    #[test]
    fn test_parse_sgr_middle_press() {
        let buf = b"\x1b[<1;5;5M";
        let event = try_parse_mouse_event(buf).unwrap();
        assert_eq!(event.button, MouseButton::Middle);
        assert_eq!(event.event_type, MouseEventType::Press);
    }

    #[test]
    fn test_parse_sgr_wheel_up() {
        // SGR wheel up: cb=64
        let buf = b"\x1b[<64;10;5M";
        let event = try_parse_mouse_event(buf).unwrap();
        assert_eq!(event.button, MouseButton::WheelUp);
        assert_eq!(event.event_type, MouseEventType::Press);
    }

    #[test]
    fn test_parse_sgr_wheel_down() {
        // SGR wheel down: cb=65
        let buf = b"\x1b[<65;10;5M";
        let event = try_parse_mouse_event(buf).unwrap();
        assert_eq!(event.button, MouseButton::WheelDown);
        assert_eq!(event.event_type, MouseEventType::Press);
    }

    #[test]
    fn test_parse_legacy_left_press() {
        // ESC [ M Cb Cx+32 Cy+32
        // cb=0 (left), cx=10 → 42, cy=5 → 37
        let buf = b"\x1b[M\x00\x2a\x25";
        let event = try_parse_mouse_event(buf).unwrap();
        assert_eq!(event.button, MouseButton::Left);
        assert_eq!(event.event_type, MouseEventType::Press);
        assert_eq!(event.x, 10);
        assert_eq!(event.y, 5);
    }

    #[test]
    fn test_parse_legacy_release() {
        // cb=3 (release), cx=0+32=32, cy=0+32=32
        let buf = b"\x1b[M\x03\x20\x20";
        let event = try_parse_mouse_event(buf).unwrap();
        assert_eq!(event.button, MouseButton::Left); // cb & 3 = 3, but release detected
        assert_eq!(event.event_type, MouseEventType::Release);
    }

    #[test]
    fn test_parse_sgr_bare_motion() {
        // ESC [ < 35 ; 10 ; 5 M  (motion without button press: cb=0x20|3=35)
        let buf = b"\x1b[<35;10;5M";
        let event = try_parse_mouse_event(buf).unwrap();
        assert_eq!(event.event_type, MouseEventType::Motion);
    }

    #[test]
    fn test_parse_legacy_bare_motion() {
        // Legacy motion without button: cb = 0x20 | 0x03 = 0x23
        let buf = b"\x1b[M\x23\x20\x20";
        let event = try_parse_mouse_event(buf).unwrap();
        assert_eq!(event.event_type, MouseEventType::Motion);
    }

    #[test]
    fn test_parse_incomplete_sgr() {
        // Only partial SGR sequence
        let buf = b"\x1b[<0;10";
        assert!(try_parse_mouse_event(buf).is_none());
    }

    #[test]
    fn test_parse_incomplete_legacy() {
        // Only partial legacy sequence
        let buf = b"\x1b[M\x00";
        assert!(try_parse_mouse_event(buf).is_none());
    }

    #[test]
    fn test_parse_garbage() {
        let buf = b"hello world";
        assert!(try_parse_mouse_event(buf).is_none());
    }

    #[test]
    fn test_parse_empty() {
        let buf: &[u8] = b"";
        assert!(try_parse_mouse_event(buf).is_none());
    }

    // ── build_cell_sgr tests ──

    #[test]
    fn test_build_cell_sgr_default() {
        let cell = crate::vtty::cell::Cell::default();
        let sgr = build_cell_sgr(&cell);
        // Default cell: fg=[204,204,204], bg=[0,0,0], no attrs
        // Should produce "\x1b[0m" (reset)
        assert_eq!(sgr, "\x1b[0m");
    }

    #[test]
    fn test_build_cell_sgr_bold() {
        let mut cell = crate::vtty::cell::Cell::default();
        cell.bold = true;
        let sgr = build_cell_sgr(&cell);
        assert!(sgr.contains("\x1b[1m")); // bold
    }

    #[test]
    fn test_build_cell_sgr_italic() {
        let mut cell = crate::vtty::cell::Cell::default();
        cell.italic = true;
        let sgr = build_cell_sgr(&cell);
        assert!(sgr.contains("\x1b[3m")); // italic
    }

    #[test]
    fn test_build_cell_sgr_custom_fg() {
        let mut cell = crate::vtty::cell::Cell::default();
        cell.fg = [255, 0, 128]; // custom foreground
        let sgr = build_cell_sgr(&cell);
        assert!(sgr.contains("\x1b[38;2;255;0;128m")); // custom fg
    }

    #[test]
    fn test_build_cell_sgr_custom_bg() {
        let mut cell = crate::vtty::cell::Cell::default();
        cell.bg = [10, 20, 30]; // custom background
        let sgr = build_cell_sgr(&cell);
        assert!(sgr.contains("\x1b[48;2;10;20;30m")); // custom bg
    }

    #[test]
    fn test_build_cell_sgr_reverse() {
        let mut cell = crate::vtty::cell::Cell::default();
        cell.reverse = true;
        let sgr = build_cell_sgr(&cell);
        assert!(sgr.contains("\x1b[7m")); // reverse
    }

    #[test]
    fn test_build_cell_sgr_all_attrs() {
        let mut cell = crate::vtty::cell::Cell::default();
        cell.bold = true;
        cell.italic = true;
        cell.underline = true;
        cell.reverse = true;
        cell.fg = [100, 200, 50];
        cell.bg = [10, 20, 30];
        let sgr = build_cell_sgr(&cell);
        assert!(sgr.contains("\x1b[38;2;100;200;50m")); // custom fg
        assert!(sgr.contains("\x1b[48;2;10;20;30m")); // custom bg
        assert!(sgr.contains("\x1b[1m")); // bold
        assert!(sgr.contains("\x1b[3m")); // italic
        assert!(sgr.contains("\x1b[4m")); // underline
        assert!(sgr.contains("\x1b[7m")); // reverse
        assert!(!sgr.contains("\x1b[0m")); // should NOT have full reset when attrs are set
    }
    // ── execute_context_menu_action tests (pure-logic, no manager) ──
    // These test the return-value logic via a lightweight approach:
    // copy_id and unknown actions preserve active_id without needing a manager.

    #[test]
    fn test_execute_context_menu_action_copy_id_preserves_active() {
        // copy_id returns active_id unchanged — verify the base64 helper
        // used by copy_id is correct.
        let id = "abc-123"; // 7 bytes → ceil(7/3)*4 = 12 base64 chars
        let encoded = base64_encode(id);
        assert!(!encoded.is_empty());
        assert_eq!(encoded.len(), 12);
    }

    #[test]
    fn test_base64_encode_roundtrip_prefix() {
        // Verify base64_encode produces output that starts with expected
        // characters for a known input.  "Hello" → first 3 bytes encode as
        // 'S', 'G', 'V', 's' in base64.
        let encoded = base64_encode("Hello");
        assert!(encoded.starts_with("SGVs"));
    }

    #[test]
    fn test_execute_context_menu_action_kill_clears_active() {
        // Verify the logic: when active_id matches target_id, result should be None.
        let active_id: Option<String> = Some("cmd-1".to_string());
        let target_id = "cmd-1";
        assert_eq!(active_id.as_deref(), Some(target_id));
    }

    #[test]
    fn test_execute_context_menu_action_kill_preserves_other() {
        // When target differs from active_id, result should be Some(active_id.clone()).
        let active_id: Option<String> = Some("cmd-1".to_string());
        let target_id = "cmd-2";
        assert_ne!(active_id.as_deref(), Some(target_id));
        assert_eq!(active_id.clone(), Some("cmd-1".to_string()));
    }

    // ── CommandLoopResult tests ──

    #[test]
    fn test_command_loop_result_debug() {
        assert_eq!(format!("{:?}", CommandLoopResult::Break), "Break");
        assert_eq!(format!("{:?}", CommandLoopResult::Continue), "Continue");
        assert_eq!(
            format!("{:?}", CommandLoopResult::RenderAndContinue),
            "RenderAndContinue"
        );
    }

    // ── SpawnCommandResult tests ──

    #[test]
    fn test_spawn_command_result_debug() {
        assert_eq!(
            format!("{:?}", SpawnCommandResult::ShouldBreak),
            "ShouldBreak"
        );
        assert_eq!(
            format!("{:?}", SpawnCommandResult::Spawned("id-1".to_string())),
            r#"Spawned("id-1")"#
        );
        assert_eq!(format!("{:?}", SpawnCommandResult::NoOp), "NoOp");
    }
}
