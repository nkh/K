use crossterm::{
    cursor::{self, MoveTo},
    style::{self, Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor, Attribute},
    terminal::{Clear, ClearType},
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
    /// Each row is explicitly positioned with MoveTo(0, row) to avoid
    /// cursor drift in raw mode, where \n moves down without returning
    /// to column 0.
    ///
    /// The renderer emits every cell in the visible area (up to render_cols)
    /// to ensure that background colors are always correct.  Relying on
    /// Clear(All) to fill background only works when the terminal's default
    /// background matches the VTTY's default bg ([0,0,0]), which is not
    /// guaranteed (e.g. solarized terminals, transparent terminals).
    pub fn render(buffer: &Buffer) -> io::Result<()> {
        let mut stdout = stdout();

        // Determine the physical terminal size and clamp to it.
        // Even though we try to match the VTTY to the terminal, the user
        // might resize the terminal between size detection and rendering,
        // or there might be a slight mismatch.  Rendering beyond the
        // physical screen causes unwanted scrolling which appears as
        // inverted/wrapped content.
        // NOTE: crossterm::terminal::size() returns (columns, rows).
        let (phys_cols, phys_rows) = crossterm::terminal::size()
            .unwrap_or((buffer.width as u16, buffer.height as u16));
        let render_rows = (buffer.rows.len() as u16).min(phys_rows) as usize;
        let render_cols = (buffer.width as u16).min(phys_cols) as usize;

        // Clear screen and move to top-left.
        // This handles any area beyond the VTTY buffer (e.g. terminal wider
        // than the VTTY) and resets leftover content from the previous frame.
        stdout.queue(Clear(ClearType::All))?;

        // Track the last rendered style to avoid redundant SGR sequences.
        let mut last_fg: Option<[u8; 3]> = None;
        let mut last_bg: Option<[u8; 3]> = None;
        let mut last_bold = false;
        let mut last_italic = false;
        let mut last_underline = false;
        let mut last_blink = false;
        let mut last_reverse = false;
        let mut last_strikethrough = false;

        // Reset style before starting so we start from a known state.
        stdout.queue(ResetColor)?;
        stdout.queue(style::SetAttribute(Attribute::Reset))?;

        for (row_idx, row) in buffer.rows.iter().enumerate().take(render_rows) {
            // Move to the start of each row — critical in raw mode
            // where \n does NOT reset the column to 0.
            stdout.queue(MoveTo(0, row_idx as u16))?;

            // Clamp the visible portion to the physical terminal width.
            let visible_len = render_cols.min(row.len());

            // Find the last non-default cell to determine how far we need
            // to render.  Cells beyond this point are guaranteed to be
            // DEFAULT_CELL, and since we cleared the screen above (which
            // fills with the terminal's default bg), they're already correct
            // IF the terminal's default bg is [0,0,0].  However, to be safe,
            // we also check if any cell has a non-default background — if so,
            // we must render it to ensure the correct bg color is displayed.
            let last_interesting = row[..visible_len]
                .iter()
                .rposition(|c| {
                    // A cell is "interesting" if it differs from DEFAULT_CELL
                    // in any way, OR if it has a non-default background
                    // (because the terminal's cleared bg may not match).
                    c != &DEFAULT_CELL
                });

            let render_end = match last_interesting {
                Some(idx) => idx + 1,
                None => continue, // Entire visible row is default — skip it
            };

            for cell in &row[..render_end] {
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

            // At the end of each row, reset the style so the next row starts clean.
            // This avoids leaking background colors to the Clear(All)-blanked area.
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

        stdout.flush()?;
        Ok(())
    }

    /// Clear the terminal screen.
    pub fn clear() -> io::Result<()> {
        let mut stdout = stdout();
        stdout.execute(Clear(ClearType::All))?;
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
