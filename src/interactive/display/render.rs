//! Rendering helpers for the interactive display loop.
//!
//! Extracted from display.rs for maintainability. Contains all VTTY rendering
//! functions: tab bar, split pane, search bar/highlights, context menu,
//! exited watermark, cell SGR building, search match finding, and log overlay.

use std::io::Write;
use std::sync::Arc;

use crate::process::manager::CommandManager;

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
            let cur_visible = handle.is_cursor_visible().await;
            drop(handle);
            let _ = TerminalDisplay::render(&buf, tab_offset, scrollback_offset);
            // Only show cursor when not scrolled back and the child
            // application has not hidden it (e.g. htop uses ?25l).
            if scrollback_offset == 0 && cur_visible {
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
            let _ = write!(stdout, "\r\n  vrc: no commands running.\r\n");
            let _ = write!(
                stdout,
                "  Waiting for commands (web UI, API, or F12 to spawn).\r\n"
            );
            let _ = write!(stdout, "\r\n  Press Ctrl+\\ to quit.\r\n");
            let _ = stdout.flush();
        }
    }
}

/// Set the background and foreground colors for a tab based on its state.
///
/// The six combinations of `is_active`, `is_frozen`, and `is_exited` map to
/// distinct color pairs used throughout the tab bar.
fn set_tab_style(
    stdout: &mut std::io::Stdout,
    is_active: bool,
    is_frozen: bool,
    is_exited: bool,
) -> std::io::Result<()> {
    use crossterm::QueueableCommand;
    use crossterm::style::{Color, SetBackgroundColor, SetForegroundColor};

    let (bg, fg) = match (is_active, is_frozen, is_exited) {
        (true, true, _) => (
            Color::Rgb { r: 80, g: 60, b: 20 },
            Color::Rgb { r: 255, g: 220, b: 100 },
        ),
        (true, false, true) => (
            Color::Rgb { r: 68, g: 71, b: 90 },
            Color::Rgb { r: 255, g: 120, b: 120 },
        ),
        (true, false, false) => (
            Color::Rgb { r: 68, g: 71, b: 90 },
            Color::Rgb { r: 255, g: 255, b: 255 },
        ),
        (false, true, _) => (
            Color::Rgb { r: 60, g: 45, b: 15 },
            Color::Rgb { r: 210, g: 180, b: 80 },
        ),
        (false, false, true) => (
            Color::Rgb { r: 40, g: 42, b: 54 },
            Color::Rgb { r: 180, g: 100, b: 100 },
        ),
        (false, false, false) => (
            Color::Rgb { r: 40, g: 42, b: 54 },
            Color::Rgb { r: 140, g: 140, b: 140 },
        ),
    };
    stdout
        .queue(SetBackgroundColor(bg))?
        .queue(SetForegroundColor(fg))?;
    Ok(())
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

        let _ = set_tab_style(&mut stdout, is_active, is_frozen, is_exited);
        stdout
            .queue(style::SetAttribute(if is_active {
                Attribute::Bold
            } else {
                Attribute::NoBold
            }))
            .ok();

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

/// Render a single pane of a split-pane view.
///
/// `start_col` is the 1-indexed column where the pane begins.
/// `pane_width` is the maximum number of columns to render.
/// When `always_clear` is true the right-edge clear is emitted unconditionally
/// (used by the right pane); otherwise it is only emitted when the row content
/// is shorter than `pane_width` (used by the left pane).
fn render_pane(
    manager: &Arc<CommandManager>,
    id: &String,
    tab_offset: u16,
    available_rows: u16,
    start_col: u16,
    pane_width: usize,
    always_clear: bool,
) {
    use std::io::Write;
    let mut stdout = std::io::stdout();
    if let Some(handle) = manager.get(id) {
        let buf = handle.vtty_snapshot_blocking();
        let render_cols = pane_width.min(buf.width);
        let total_lines = buf.total_lines();
        let viewport_start = total_lines.saturating_sub(available_rows as usize);
        let mut last_sgr = String::new();
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
                start_col
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
            // Clear remaining cells in this row within the pane
            if always_clear || (visible_len as u16) < pane_width as u16 {
                let _ = write!(stdout, "\x1b[0m\x1b[K");
                last_sgr = String::new();
            }
        }
        // Show pane label
        let label = format!(" {} ", id);
        let _ = write!(
            stdout,
            "\x1b[1;{}H\x1b[48;5;238m\x1b[38;5;255m{}\x1b[0m",
            start_col,
            label
        );
    }
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

    // Render left pane (column 1, width = half_col, conditional clear)
    if let Some(ref id) = left_id {
        render_pane(manager, id, tab_offset, available_rows, 1, div_col, false);
    }

    // Render right pane (column half_col + 2, width = phys_cols - half_col - 1, always clear)
    if let Some(ref id) = right_id {
        let right_start = (half_col + 2) as u16;
        let right_width = (phys_cols - half_col as u16 - 1) as usize;
        render_pane(manager, id, tab_offset, available_rows, right_start, right_width, true);
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

/// Render the command log as an overlay on top of the VTTY display.
/// Shows the most recent log entries, with the newest at the bottom.
pub fn render_log_overlay(
    _manager: &Arc<CommandManager>,
    log_entries: &Arc<std::sync::Mutex<std::collections::VecDeque<String>>>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_cell_sgr_default() {
        use crate::vtty::cell::Cell;
        let cell = Cell {
            ch: ' ',
            fg: [204, 204, 204],
            bg: [0, 0, 0],
            bold: false, italic: false, underline: false,
            blink: false, reverse: false, invisible: false, strikethrough: false,
            width: 1,
        };
        let sgr = build_cell_sgr(&cell);
        assert!(!sgr.contains("[38;2;"));
        assert!(!sgr.contains("[48;2;"));
    }

    #[test]
    fn test_build_cell_sgr_custom_fg() {
        use crate::vtty::cell::Cell;
        let cell = Cell {
            ch: 'A',
            fg: [255, 0, 0],
            bg: [0, 0, 0],
            bold: false, italic: false, underline: false,
            blink: false, reverse: false, invisible: false, strikethrough: false,
            width: 1,
        };
        let sgr = build_cell_sgr(&cell);
        assert!(sgr.contains("38;2;255;0;0"));
    }

    #[test]
    fn test_build_cell_sgr_custom_bg() {
        use crate::vtty::cell::Cell;
        let cell = Cell {
            ch: 'A',
            fg: [204, 204, 204],
            bg: [0, 0, 255],
            bold: false, italic: false, underline: false,
            blink: false, reverse: false, invisible: false, strikethrough: false,
            width: 1,
        };
        let sgr = build_cell_sgr(&cell);
        assert!(sgr.contains("48;2;0;0;255"));
    }

    #[test]
    fn test_build_cell_sgr_bold() {
        use crate::vtty::cell::Cell;
        let cell = Cell {
            ch: 'A',
            fg: [204, 204, 204],
            bg: [0, 0, 0],
            bold: true, italic: false, underline: false,
            blink: false, reverse: false, invisible: false, strikethrough: false,
            width: 1,
        };
        let sgr = build_cell_sgr(&cell);
        assert!(sgr.contains("1m"));
    }

    #[test]
    fn test_build_cell_sgr_all_attributes() {
        use crate::vtty::cell::Cell;
        let cell = Cell {
            ch: 'X',
            fg: [128, 128, 128],
            bg: [64, 64, 64],
            bold: true, italic: true, underline: true,
            blink: true, reverse: true, invisible: false, strikethrough: true,
            width: 1,
        };
        let sgr = build_cell_sgr(&cell);
        assert!(sgr.contains("38;2;128;128;128"));
        assert!(sgr.contains("48;2;64;64;64"));
        assert!(sgr.contains("1m"));  // bold
        assert!(sgr.contains("3m"));  // italic
        assert!(sgr.contains("4m"));  // underline
        assert!(sgr.contains("7m"));  // reverse
        // Note: blink, invisible, strikethrough are not rendered by build_cell_sgr
    }
}