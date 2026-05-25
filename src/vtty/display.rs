use crossterm::{
    cursor::{self, MoveTo},
    style::{self, Attribute, Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor},
    terminal::ClearType,
    ExecutableCommand, QueueableCommand,
};
use std::io::{self, stdout, Write};

use super::buffer::Buffer;
use super::cell::Cell;
use super::emulator::CursorStyle;

/// VTTY default foreground: light grey.
const DEFAULT_FG: [u8; 3] = [204, 204, 204];

/// VTTY default background: black.
const DEFAULT_BG: [u8; 3] = [0, 0, 0];

/// A fully-default cell used to detect cells that don't need rendering.
const DEFAULT_CELL: Cell = Cell {
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

/// Renders a VTTY buffer to the local terminal using crossterm.
pub struct TerminalDisplay;

impl TerminalDisplay {
    /// Render the buffer to stdout with full color support.
    ///
    /// Cells whose fg/bg match the VTTY defaults (fg=[204,204,204],
    /// bg=[0,0,0]) are treated as "terminal default" colours — the
    /// display emits `ESC[39m` / `ESC[49m` so that the user's own
    /// terminal colour scheme (solarized, transparent, custom palette)
    /// passes through.  Only explicitly-set colours are rendered as
    /// RGB values.
    ///
    /// Each row is explicitly positioned with MoveTo(0, row) to avoid
    /// cursor drift in raw mode, where \n moves down without returning
    /// to column 0.
    ///
    /// `row_offset` shifts all output down by the given number of rows,
    /// used to make room for a tab bar at the top of the terminal.
    pub fn render(buffer: &Buffer, row_offset: u16) -> io::Result<()> {
        let mut stdout = stdout();

        // Determine the physical terminal size and clamp to it.
        // NOTE: crossterm::terminal::size() returns (columns, rows).
        let (phys_cols, phys_rows) = crossterm::terminal::size()
            .unwrap_or((buffer.width as u16, buffer.height as u16));
        let available_rows = phys_rows.saturating_sub(row_offset);
        let render_rows = (buffer.rows.len() as u16).min(available_rows) as usize;
        let render_cols = (buffer.width as u16).min(phys_cols) as usize;

        // Track the last rendered style to avoid redundant SGR sequences.
        // `None` means "terminal default" (ESC[39m / ESC[49m).
        let mut last_fg: Option<[u8; 3]> = None;
        let mut last_bg: Option<[u8; 3]> = None;
        let mut last_bold = false;
        let mut last_italic = false;
        let mut last_underline = false;
        let mut last_blink = false;
        let mut last_reverse = false;
        let mut last_strikethrough = false;

        for (row_idx, row) in buffer.rows.iter().enumerate().take(render_rows) {
            // Move to the start of each row — critical in raw mode
            // where \n does NOT reset the column to 0.
            // Apply row_offset so VTTY content starts below the tab bar.
            stdout.queue(MoveTo(0, row_idx as u16 + row_offset))?;

            // Clamp the visible portion to the physical terminal width.
            let visible_len = render_cols.min(row.len());

            for cell in &row[..visible_len] {
                // Fast path: cell is fully default AND we are already in
                // the default terminal state — no SGR needed at all.
                if cell == &DEFAULT_CELL && last_fg.is_none() && last_bg.is_none()
                    && !last_bold && !last_italic && !last_underline
                    && !last_blink && !last_reverse && !last_strikethrough
                {
                    stdout.queue(Print(cell.ch))?;
                    continue;
                }

                // ── Foreground ──
                // When the cell fg matches the VTTY default we emit
                // ESC[39m ("default fg") so the terminal uses its own
                // palette instead of forcing RGB [204,204,204].
                let cell_fg = if cell.fg == DEFAULT_FG { None } else { Some(cell.fg) };
                if cell_fg != last_fg {
                    if let Some(rgb) = cell_fg {
                        stdout.queue(SetForegroundColor(Color::Rgb {
                            r: rgb[0], g: rgb[1], b: rgb[2],
                        }))?;
                    } else {
                        stdout.queue(Print("\x1b[39m"))?;
                    }
                    last_fg = cell_fg;
                }

                // ── Background ──
                // Same strategy as foreground: default bg → ESC[49m.
                let cell_bg = if cell.bg == DEFAULT_BG { None } else { Some(cell.bg) };
                if cell_bg != last_bg {
                    if let Some(rgb) = cell_bg {
                        stdout.queue(SetBackgroundColor(Color::Rgb {
                            r: rgb[0], g: rgb[1], b: rgb[2],
                        }))?;
                    } else {
                        stdout.queue(Print("\x1b[49m"))?;
                    }
                    last_bg = cell_bg;
                }

                // ── Text attributes ──
                if cell.bold != last_bold {
                    stdout.queue(style::SetAttribute(if cell.bold { Attribute::Bold } else { Attribute::NoBold }))?;
                    last_bold = cell.bold;
                }
                if cell.italic != last_italic {
                    stdout.queue(style::SetAttribute(if cell.italic { Attribute::Italic } else { Attribute::NoItalic }))?;
                    last_italic = cell.italic;
                }
                if cell.underline != last_underline {
                    stdout.queue(style::SetAttribute(if cell.underline { Attribute::Underlined } else { Attribute::NoUnderline }))?;
                    last_underline = cell.underline;
                }
                if cell.blink != last_blink {
                    stdout.queue(style::SetAttribute(if cell.blink { Attribute::SlowBlink } else { Attribute::NoBlink }))?;
                    last_blink = cell.blink;
                }
                if cell.reverse != last_reverse {
                    stdout.queue(style::SetAttribute(if cell.reverse { Attribute::Reverse } else { Attribute::NoReverse }))?;
                    last_reverse = cell.reverse;
                }
                if cell.strikethrough != last_strikethrough {
                    stdout.queue(style::SetAttribute(if cell.strikethrough { Attribute::CrossedOut } else { Attribute::NotCrossedOut }))?;
                    last_strikethrough = cell.strikethrough;
                }

                // Print the character
                stdout.queue(Print(cell.ch))?;
            }

            // Clear any remaining columns on this row if the terminal
            // is wider than the VTTY buffer.  Reset to terminal defaults
            // so the cleared area uses the user's own bg colour.
            if (visible_len as u16) < phys_cols {
                stdout.queue(ResetColor)?;
                stdout.queue(style::SetAttribute(Attribute::Reset))?;
                stdout.queue(crossterm::terminal::Clear(ClearType::UntilNewLine))?;
                last_fg = None;
                last_bg = None;
                last_bold = false;
                last_italic = false;
                last_underline = false;
                last_blink = false;
                last_reverse = false;
                last_strikethrough = false;
            } else {
                // At the end of each row, reset the style so the next
                // row starts clean.  This avoids leaking background
                // colors across rows.
                if last_fg.is_some() || last_bg.is_some()
                    || last_bold || last_italic || last_underline
                    || last_blink || last_reverse || last_strikethrough
                {
                    stdout.queue(ResetColor)?;
                    stdout.queue(style::SetAttribute(Attribute::Reset))?;
                    last_fg = None;
                    last_bg = None;
                    last_bold = false;
                    last_italic = false;
                    last_underline = false;
                    last_blink = false;
                    last_reverse = false;
                    last_strikethrough = false;
                }
            }
        }

        // Clear any remaining rows below the VTTY buffer if the
        // terminal is taller than the VTTY.
        if (render_rows as u16 + row_offset) < phys_rows {
            for row in render_rows..(available_rows as usize) {
                stdout.queue(MoveTo(0, row as u16 + row_offset))?;
                stdout.queue(crossterm::terminal::Clear(ClearType::UntilNewLine))?;
            }
        }

        stdout.flush()?;
        Ok(())
    }

    /// Clear the terminal screen.
    pub fn clear() -> io::Result<()> {
        let mut stdout = stdout();
        stdout.execute(crossterm::terminal::Clear(ClearType::All))?;
        stdout.execute(MoveTo(0, 0))?;
        Ok(())
    }

    /// Hide the cursor.
    pub fn hide_cursor() -> io::Result<()> {
        stdout().execute(cursor::Hide)?;
        Ok(())
    }

    /// Show a steady (non-blinking) block cursor at the given position.
    pub fn show_cursor_at(row: usize, col: usize) -> io::Result<()> {
        let mut stdout = stdout();
        // DEC private mode 12: disable blinking cursor
        stdout.queue(Print("\x1b[?12l"))?;
        // DEC private mode 25: show cursor
        stdout.queue(cursor::Show)?;
        stdout.queue(MoveTo(col as u16, row as u16))?;
        stdout.flush()?;
        Ok(())
    }

    /// Show the cursor (uses terminal default style).
    pub fn show_cursor() -> io::Result<()> {
        stdout().execute(cursor::Show)?;
        Ok(())
    }

    /// Show cursor with the specified style.
    /// `style` determines the shape (block/underline/bar) and blink state.
    pub fn show_cursor_with_style(row: usize, col: usize, style: CursorStyle) -> io::Result<()> {
        let mut stdout = stdout();
        // DECSCUSR: set cursor style in the hosting terminal
        let ps: u8 = match style {
            CursorStyle::Block(blink) => if blink { 1 } else { 2 },
            CursorStyle::Underline(blink) => if blink { 3 } else { 4 },
            CursorStyle::Bar(blink) => if blink { 5 } else { 6 },
        };
        stdout.queue(Print(format!("\x1b[{} q", ps)))?;
        stdout.queue(cursor::Show)?;
        stdout.queue(MoveTo(col as u16, row as u16))?;
        stdout.flush()?;
        Ok(())
    }
}
