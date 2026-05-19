use crossterm::{
    cursor,
    terminal,
    ExecutableCommand,
    QueueableCommand,
};
use std::io::{self, Write};

use super::buffer::Buffer;

pub struct TerminalDisplay;

impl TerminalDisplay {
    pub fn render(buffer: &Buffer) -> io::Result<()> {
        let mut stdout = io::stdout();
        stdout.queue(terminal::Clear(terminal::ClearType::All))?;
        stdout.queue(cursor::MoveTo(0, 0))?;

        for row in &buffer.rows {
            for cell in row {
                // TODO: Map cell colors to crossterm SetForegroundColor/SetBackgroundColor
                print!("{}", cell.ch);
            }
            println!();
        }
        stdout.flush()?;
        Ok(())
    }
}
