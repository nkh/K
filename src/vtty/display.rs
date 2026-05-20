use crossterm::{
    cursor::{self, MoveTo},
    style::{self, Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor, Attribute},
    terminal::{Clear, ClearType},
    ExecutableCommand, QueueableCommand,
};
use std::io::{self, stdout, Write};

use super::buffer::Buffer;
use super::cell::Cell;

/// Renders a VTTY buffer to the local terminal using crossterm.
pub struct TerminalDisplay;

impl TerminalDisplay {
    /// Render the buffer to stdout with full color support.
    ///
    /// Each row is explicitly positioned with MoveTo(0, row) to avoid
    /// cursor drift in raw mode, where \n moves down without returning
    /// to column 0.  Output is clipped to the physical terminal dimensions
    /// so that a VTTY buffer wider/taller than the visible area does not
    /// cause wrapping or spurious line breaks.
    pub fn render(buffer: &Buffer) -> io::Result<()> {
        let mut stdout = stdout();

        // Query the physical terminal size and clip to it.
        let (term_rows, term_cols) = crossterm::terminal::size()
            .unwrap_or((buffer.rows.len() as u16, buffer.width as u16));

        // Clear screen and move to top-left
        stdout.queue(Clear(ClearType::All))?;

        let render_rows = (buffer.rows.len() as u16).min(term_rows) as usize;
        let render_cols = (buffer.width as u16).min(term_cols) as usize;

        for (row_idx, row) in buffer.rows.iter().enumerate().take(render_rows) {
            // Move to the start of each row — critical in raw mode
            // where \n does NOT reset the column to 0.
            stdout.queue(MoveTo(0, row_idx as u16))?;
            for cell in row.iter().take(render_cols) {
                Self::render_cell(&mut stdout, cell)?;
            }
        }

        stdout.flush()?;
        Ok(())
    }

    /// Render a single cell with its attributes.
    fn render_cell(stdout: &mut io::Stdout, cell: &Cell) -> io::Result<()> {
        // Set foreground color
        let fg = Color::Rgb {
            r: cell.fg[0],
            g: cell.fg[1],
            b: cell.fg[2],
        };
        stdout.queue(SetForegroundColor(fg))?;

        // Set background color
        let bg = Color::Rgb {
            r: cell.bg[0],
            g: cell.bg[1],
            b: cell.bg[2],
        };
        stdout.queue(SetBackgroundColor(bg))?;

        // Apply attributes
        if cell.bold {
            stdout.queue(style::SetAttribute(Attribute::Bold))?;
        }
        if cell.italic {
            stdout.queue(style::SetAttribute(Attribute::Italic))?;
        }
        if cell.underline {
            stdout.queue(style::SetAttribute(Attribute::Underlined))?;
        }
        if cell.blink {
            stdout.queue(style::SetAttribute(Attribute::SlowBlink))?;
        }
        if cell.reverse {
            stdout.queue(style::SetAttribute(Attribute::Reverse))?;
        }
        if cell.strikethrough {
            stdout.queue(style::SetAttribute(Attribute::CrossedOut))?;
        }

        // Print the character
        stdout.queue(Print(cell.ch))?;

        // Reset attributes for next cell
        stdout.queue(ResetColor)?;
        stdout.queue(style::SetAttribute(Attribute::Reset))?;

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
