use super::cell::Cell;

#[derive(Debug, Clone)]
pub struct Buffer {
    pub rows: Vec<Vec<Cell>>,
    pub scrollback: Vec<Vec<Cell>>,
    pub width: usize,
    pub height: usize,
    max_scrollback: usize,
}

impl Buffer {
    pub fn new(width: usize, height: usize, max_scrollback: usize) -> Self {
        Self {
            rows: vec![vec![Cell::default(); width]; height],
            scrollback: Vec::new(),
            width,
            height,
            max_scrollback,
        }
    }

    pub fn resize(&mut self, new_width: usize, new_height: usize) {
        for row in &mut self.rows {
            row.resize(new_width, Cell::default());
        }
        self.rows.resize(new_height, vec![Cell::default(); new_width]);
        self.width = new_width;
        self.height = new_height;
    }

    pub fn get(&self, row: usize, col: usize) -> Option<&Cell> {
        self.rows.get(row)?.get(col)
    }

    pub fn get_mut(&mut self, row: usize, col: usize) -> Option<&mut Cell> {
        self.rows.get_mut(row)?.get_mut(col)
    }

    pub fn set(&mut self, row: usize, col: usize, cell: Cell) {
        if let Some(c) = self.get_mut(row, col) {
            *c = cell;
        }
    }

    pub fn clear_all(&mut self) {
        for row in &mut self.rows {
            for cell in row {
                cell.clear();
            }
        }
    }

    pub fn clear_line_from(&mut self, row: usize, col: usize) {
        if let Some(row_cells) = self.rows.get_mut(row) {
            for cell in row_cells.iter_mut().skip(col) {
                cell.clear();
            }
        }
    }

    pub fn clear_line_to(&mut self, row: usize, col: usize) {
        if let Some(row_cells) = self.rows.get_mut(row) {
            for cell in row_cells.iter_mut().take(col + 1) {
                cell.clear();
            }
        }
    }

    pub fn clear_line(&mut self, row: usize) {
        if let Some(row_cells) = self.rows.get_mut(row) {
            for cell in row_cells {
                cell.clear();
            }
        }
    }

    pub fn clear_screen_from(&mut self, start_row: usize, start_col: usize) {
        self.clear_line_from(start_row, start_col);
        for row in (start_row + 1)..self.height {
            self.clear_line(row);
        }
    }

    pub fn clear_screen_to(&mut self, end_row: usize, end_col: usize) {
        for row in 0..end_row {
            self.clear_line(row);
        }
        self.clear_line_to(end_row, end_col);
    }

    pub fn scroll_up(&mut self) {
        if !self.rows.is_empty() {
            let removed = self.rows.remove(0);
            if self.scrollback.len() < self.max_scrollback {
                self.scrollback.push(removed);
            } else if !self.scrollback.is_empty() {
                self.scrollback.remove(0);
                self.scrollback.push(removed);
            }
            self.rows.push(vec![Cell::default(); self.width]);
        }
    }

    pub fn scroll_down(&mut self) {
        if !self.rows.is_empty() {
            self.rows.pop();
            self.rows.insert(0, vec![Cell::default(); self.width]);
        }
    }

    pub fn insert_line(&mut self, row: usize) {
        if row < self.height {
            self.rows.insert(row, vec![Cell::default(); self.width]);
            self.rows.pop();
        }
    }

    pub fn delete_line(&mut self, row: usize) {
        if row < self.height {
            self.rows.remove(row);
            self.rows.push(vec![Cell::default(); self.width]);
        }
    }

    pub fn insert_cells(&mut self, row: usize, col: usize, count: usize) {
        if let Some(row_cells) = self.rows.get_mut(row) {
            let count = count.min(self.width - col);
            for i in (col + count..self.width).rev() {
                row_cells[i] = row_cells[i - count];
            }
            for cell in row_cells.iter_mut().skip(col).take(count) {
                cell.clear();
            }
        }
    }

    pub fn delete_cells(&mut self, row: usize, col: usize, count: usize) {
        if let Some(row_cells) = self.rows.get_mut(row) {
            let count = count.min(self.width - col);
            for i in col..(self.width - count) {
                row_cells[i] = row_cells[i + count];
            }
            for cell in row_cells.iter_mut().take(self.width).skip(self.width - count) {
                cell.clear();
            }
        }
    }

    pub fn total_lines(&self) -> usize {
        self.scrollback.len() + self.rows.len()
    }

    pub fn get_line(&self, index: usize) -> Option<&Vec<Cell>> {
        if index < self.scrollback.len() {
            self.scrollback.get(index)
        } else {
            self.rows.get(index - self.scrollback.len())
        }
    }

    pub fn get_lines(&self, start: usize, end: usize) -> Vec<&Vec<Cell>> {
        (start..end.min(self.total_lines()))
            .filter_map(|i| self.get_line(i))
            .collect()
    }
}

/// A single changed cell in a buffer diff.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CellDiff {
    pub row: usize,
    pub col: usize,
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

/// Result of diffing two buffers.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BufferDiff {
    pub width: usize,
    pub height: usize,
    pub cells: Vec<CellDiff>,
    pub changed_count: usize,
}

impl Buffer {
    /// Compute a cell-level diff between `self` and `other`.
    /// Returns the list of cells that differ (by position).
    /// If the dimensions differ, all cells are considered changed.
    pub fn diff(&self, other: &Buffer) -> BufferDiff {
        let mut cells = Vec::new();

        if self.width != other.width || self.height != other.height {
            // Dimensions changed — return all cells from self as changed
            for row in 0..self.height {
                for col in 0..self.width {
                    let c = self.get(row, col).cloned().unwrap_or_default();
                    cells.push(CellDiff {
                        row,
                        col,
                        ch: c.ch,
                        fg: c.fg,
                        bg: c.bg,
                        bold: c.bold,
                        italic: c.italic,
                        underline: c.underline,
                        blink: c.blink,
                        reverse: c.reverse,
                        invisible: c.invisible,
                        strikethrough: c.strikethrough,
                    });
                }
            }
            return BufferDiff {
                width: self.width,
                height: self.height,
                changed_count: cells.len(),
                cells,
            };
        }

        // Same dimensions — compare cell by cell
        for row in 0..self.height {
            for col in 0..self.width {
                if let Some(a) = self.get(row, col) {
                    if let Some(b) = other.get(row, col) {
                        if a != b {
                            cells.push(CellDiff {
                                row,
                                col,
                                ch: a.ch,
                                fg: a.fg,
                                bg: a.bg,
                                bold: a.bold,
                                italic: a.italic,
                                underline: a.underline,
                                blink: a.blink,
                                reverse: a.reverse,
                                invisible: a.invisible,
                                strikethrough: a.strikethrough,
                            });
                        }
                    }
                }
            }
        }

        BufferDiff {
            width: self.width,
            height: self.height,
            changed_count: cells.len(),
            cells,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_new() {
        let b = Buffer::new(80, 24, 5000);
        assert_eq!(b.width, 80);
        assert_eq!(b.height, 24);
    }

    #[test]
    fn test_buffer_scroll() {
        let mut b = Buffer::new(10, 3, 100);
        b.rows[0][0].ch = 'A';
        b.scroll_up();
        assert_eq!(b.scrollback.len(), 1);
        assert_eq!(b.scrollback[0][0].ch, 'A');
        assert_eq!(b.rows[0][0].ch, ' ');
    }

    #[test]
    fn test_buffer_resize() {
        let mut b = Buffer::new(10, 5, 100);
        b.rows[0][0].ch = 'X';
        b.resize(20, 10);
        assert_eq!(b.width, 20);
        assert_eq!(b.height, 10);
        assert_eq!(b.rows[0][0].ch, 'X');
    }

    #[test]
    fn test_buffer_insert_delete_line() {
        let mut b = Buffer::new(10, 5, 100);
        b.rows[1][0].ch = 'B';
        b.insert_line(1);
        assert_eq!(b.rows[1][0].ch, ' ');
        assert_eq!(b.rows[2][0].ch, 'B');
        b.delete_line(1);
        assert_eq!(b.rows[1][0].ch, 'B');
    }

    #[test]
    fn test_buffer_clear_operations() {
        let mut b = Buffer::new(10, 5, 100);
        b.rows[2][5].ch = 'X';
        b.clear_screen_from(2, 5);
        assert_eq!(b.rows[2][5].ch, ' ');
        assert_eq!(b.rows[3][0].ch, ' ');
    }

    #[test]
    fn test_buffer_total_lines() {
        let mut b = Buffer::new(10, 5, 100);
        b.scroll_up();
        b.scroll_up();
        assert_eq!(b.total_lines(), 7);
    }
}
