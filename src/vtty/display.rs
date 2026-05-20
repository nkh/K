use crossterm::{
    cursor::{self, MoveTo},
    style::{self, Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor, Attribute},
    terminal::ClearType,
    ExecutableCommand, QueueableCommand,
};
use std::io::{self, stdout, Write};

use super::buffer::Buffer;
use super::cell::Cell;

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
};

/// Renders a VTTY buffer to the local terminal using crossterm.
pub struct TerminalDisplay;

impl TerminalDisplay {
    /// Render the buffer to stdout with full color support.
    ///
    /// Every cell in the visible area is rendered explicitly to guarantee
    /// that background colors are always correct, regardless of the
    /// terminal's own default background color.  This avoids a class of
    /// bugs where `Clear(All)` fills with the terminal's theme color
    /// (e.g. solarized, transparent) while the VTTY expects [0,0,0].
    ///
    /// Each row is explicitly positioned with MoveTo(0, row) to avoid
    /// cursor drift in raw mode, where \n moves down without returning
    /// to column 0.
    pub fn render(buffer: &Buffer) -> io::Result<()> {
        let mut stdout = stdout();

        // Determine the physical terminal size and clamp to it.
        // NOTE: crossterm::terminal::size() returns (columns, rows).
        let (phys_cols, phys_rows) = crossterm::terminal::size()
            .unwrap_or((buffer.width as u16, buffer.height as u16));
        let render_rows = (buffer.rows.len() as u16).min(phys_rows) as usize;
        let render_cols = (buffer.width as u16).min(phys_cols) as usize;

        // Track the last rendered style to avoid redundant SGR sequences.
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
            stdout.queue(MoveTo(0, row_idx as u16))?;

            // Clamp the visible portion to the physical terminal width.
            let visible_len = render_cols.min(row.len());

            // Render EVERY cell in the visible range.
            // We cannot skip "default" cells because:
            //   - The terminal's default background may not match
            //     the VTTY's default [0,0,0] (e.g. solarized terminals).
            //   - ESC[K fills cells with the current SGR background,
            //     which may be non-default, and those must be rendered.
            //   - Programs like htop rely on background colors being
            //     correct for every cell, including trailing spaces.
            //
            // The cost is ~2000 SGR-set + Print calls per 80x24 frame,
            // which is negligible at 100ms refresh intervals.
            for cell in &row[..visible_len] {
                // Optimization: if this cell IS the default, AND the
                // terminal's bg is [0,0,0] (which we assume for now),
                // we could skip it.  But since we can't reliably know
                // the terminal's bg, we always emit.  To keep performance
                // acceptable, we at least skip SGR emission when the
                // style hasn't changed.
                if cell == &DEFAULT_CELL && last_fg.is_none() && last_bg.is_none()
                    && !last_bold && !last_italic && !last_underline
                    && !last_blink && !last_reverse && !last_strikethrough
                {
                    // Already in default state, just print the space
                    stdout.queue(Print(' '))?;
                    continue;
                }

                // Set foreground color (only if changed)
                if Some(cell.fg) != last_fg {
                    stdout.queue(SetForegroundColor(Color::Rgb {
                        r: cell.fg[0],
                        g: cell.fg[1],
                        b: cell.fg[2],
                    }))?;
                    last_fg = Some(cell.fg);
                }

                // Set background color (only if changed)
                if Some(cell.bg) != last_bg {
                    stdout.queue(SetBackgroundColor(Color::Rgb {
                        r: cell.bg[0],
                        g: cell.bg[1],
                        b: cell.bg[2],
                    }))?;
                    last_bg = Some(cell.bg);
                }

                // Apply attributes (only if changed)
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
            // is wider than the VTTY buffer.  This ensures the area
            // to the right of the VTTY content shows the VTTY's default
            // bg color, not leftover content from the previous frame.
            if (visible_len as u16) < phys_cols {
                // Reset to default style and clear to end of line
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
        if (render_rows as u16) < phys_rows {
            for row in render_rows..(phys_rows as usize) {
                stdout.queue(MoveTo(0, row as u16))?;
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

    /// Show the cursor.
    pub fn show_cursor() -> io::Result<()> {
        stdout().execute(cursor::Show)?;
        Ok(())
    }
}
