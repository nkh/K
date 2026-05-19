use crossterm::{
    cursor::{self, MoveTo},
    style::{self, Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor, Attribute},
    terminal::{self, Clear, ClearType},
    ExecutableCommand, QueueableCommand,
};
use std::io::{self, stdout, Write};

use super::buffer::Buffer;
use super::cell::Cell;

/// Renders a VTTY buffer to the local terminal using crossterm.
pub struct TerminalDisplay;

impl TerminalDisplay {
    /// Render the buffer to stdout with full color support.
    pub fn render(buffer: &Buffer) -> io::Result<()> {
        let mut stdout = stdout();

        // Clear screen and move to top-left
        stdout.queue(Clear(ClearType::All))?;
        stdout.queue(MoveTo(0, 0))?;

        for (row_idx, row) in buffer.rows.iter().enumerate() {
            for cell in row {
                Self::render_cell(&mut stdout, cell)?;
            }
            // Only move to next line if not the last row
            if row_idx < buffer.rows.len() - 1 {
                stdout.queue(Print("\n"))?;
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
