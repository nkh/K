use serde::{Deserialize, Serialize};

/// A single cell in the terminal buffer.
/// Each cell stores a Unicode character and its visual attributes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cell {
    pub ch: char,
    pub fg: [u8; 3],
    pub bg: [u8; 3],
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub blink: bool,
    pub reverse: bool,
    pub invisible: bool,
    pub strikethrough: bool,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
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
        }
    }
}

impl Cell {
    pub fn new(ch: char) -> Self {
        Self { ch, ..Default::default() }
    }

    pub fn with_colors(ch: char, fg: [u8; 3], bg: [u8; 3]) -> Self {
        Self { ch, fg, bg, ..Default::default() }
    }

    pub fn reset_attrs(&mut self) {
        self.fg = [204, 204, 204];
        self.bg = [0, 0, 0];
        self.bold = false;
        self.italic = false;
        self.underline = false;
        self.blink = false;
        self.reverse = false;
        self.invisible = false;
        self.strikethrough = false;
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn is_empty(&self) -> bool {
        self.ch == ' '
            && self.fg == [204, 204, 204]
            && self.bg == [0, 0, 0]
            && !self.bold
            && !self.italic
            && !self.underline
            && !self.blink
            && !self.reverse
            && !self.invisible
            && !self.strikethrough
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cell_default() {
        let c = Cell::default();
        assert_eq!(c.ch, ' ');
        assert_eq!(c.fg, [204, 204, 204]);
        assert!(!c.bold);
    }

    #[test]
    fn test_cell_new() {
        let c = Cell::new('X');
        assert_eq!(c.ch, 'X');
        assert_eq!(c.fg, [204, 204, 204]);
    }

    #[test]
    fn test_cell_clear() {
        let mut c = Cell::with_colors('A', [255, 0, 0], [0, 0, 0]);
        c.bold = true;
        c.clear();
        assert_eq!(c.ch, ' ');
        assert!(!c.bold);
    }

    #[test]
    fn test_cell_is_empty() {
        assert!(Cell::default().is_empty());
        let mut c = Cell::default();
        c.ch = 'A';
        assert!(!c.is_empty());
    }
}
