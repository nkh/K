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
    /// Optimization: only emits ANSI style sequences when the cell differs
    /// from the previous one, and skips runs of default cells at the end of
    /// each row (since Clear(All) already blanked the screen).
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

        // Clear screen and move to top-left
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

        for (row_idx, row) in buffer.rows.iter().enumerate().take(render_rows) {
            // Move to the start of each row — critical in raw mode
            // where \n does NOT reset the column to 0.
            stdout.queue(MoveTo(0, row_idx as u16))?;

            // Find the last non-default cell to avoid rendering trailing spaces.
            // Since Clear(All) blanked the screen, trailing defaults are already
            // correct and emitting them just wastes bandwidth.
            // Clamp the search to render_cols to avoid reading beyond the visible area.
            let visible_row = &row[..render_cols.min(row.len())];
            let last_non_default = visible_row.iter()
                .rposition(|c| c != &DEFAULT_CELL);

            let render_end = match last_non_default {
                Some(idx) => idx + 1,
                None => continue, // Entire row is default — skip it
            };

            // Clamp render_end to the visible columns
            let render_end = render_end.min(render_cols);

            for cell in &row[..render_end] {
                let is_default = *cell == DEFAULT_CELL;

                if is_default {
                    // If the current style is non-default, we still need to reset
                    // and print a space to clear any previous content at this
                    // position.  Only skip if we already know the style is default.
                    let style_is_default = last_fg.is_none()
                        && last_bg.is_none()
                        && !last_bold && !last_italic && !last_underline
                        && !last_blink && !last_reverse && !last_strikethrough;

                    if style_is_default {
                        // Fast path: style is already default, cell is default,
                        // screen was cleared.  Just advance the cursor by printing
                        // a space (we need the space because a previous cell on
                        // this row may have set a background color).
                        // Actually, we still need to reset the background in case
                        // a previous cell had a non-default bg.  Check:
                        if last_bg.is_none() {
                            stdout.queue(Print(' '))?;
                            continue;
                        }
                    }
                    // Fall through to full rendering for the reset.
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
