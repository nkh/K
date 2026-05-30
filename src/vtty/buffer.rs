use super::cell::Cell;

#[derive(Debug, Clone)]
pub struct Buffer {
    pub rows: Vec<Vec<Cell>>,
    pub scrollback: Vec<Vec<Cell>>,
    pub width: usize,
    pub height: usize,
    max_scrollback: usize,
    /// Monotonically increasing counter incremented on every buffer mutation.
    /// Used for cheap change detection — callers compare this value instead
    /// of cloning the entire buffer and comparing cell by cell.
    /// Wraps on overflow (u64::MAX → 0) which is safe for equality checks.
    generation: u64,
}

impl Buffer {
    pub fn new(width: usize, height: usize, max_scrollback: usize) -> Self {
        Self {
            rows: vec![vec![Cell::default(); width]; height],
            scrollback: Vec::new(),
            width,
            height,
            max_scrollback,
            generation: 0,
        }
    }

    /// Get the current generation counter value.
    ///
    /// Used by [`CommandManager::has_changed`](crate::process::manager::CommandManager::has_changed)
    /// and the diff watcher for O(1) change detection without cloning the buffer.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Increment the generation counter. Called internally after every mutation.
    fn bump_generation(&mut self) {
        self.generation = self.generation.wrapping_add(1);
    }

    pub fn resize(&mut self, new_width: usize, new_height: usize) {
        for row in &mut self.rows {
            row.resize(new_width, Cell::default());
        }
        // Scrollback rows must also be resized to match the new width,
        // otherwise to_html_scrollback renders them with the wrong number
        // of columns, causing alignment errors and blank lines.
        for row in &mut self.scrollback {
            row.resize(new_width, Cell::default());
        }
        self.rows
            .resize(new_height, vec![Cell::default(); new_width]);
        self.width = new_width;
        self.height = new_height;
        self.bump_generation();
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
            self.bump_generation();
        }
    }

    /// Clear the entire buffer to default cells.
    pub fn clear_all(&mut self) {
        for row in &mut self.rows {
            for cell in row {
                cell.clear();
            }
        }
        self.bump_generation();
    }

    /// Clear the entire buffer using a template cell (respects current SGR attributes).
    /// The template's character is replaced with a space.
    pub fn clear_all_with(&mut self, template: &Cell) {
        let blank = Cell {
            ch: ' ',
            ..*template
        };
        for row in &mut self.rows {
            for cell in row {
                *cell = blank;
            }
        }
        self.bump_generation();
    }

    /// Clear from the given column to end of line.
    pub fn clear_line_from(&mut self, row: usize, col: usize) {
        if let Some(row_cells) = self.rows.get_mut(row) {
            for cell in row_cells.iter_mut().skip(col) {
                cell.clear();
            }
            self.bump_generation();
        }
    }

    /// Clear from the given column to end of line, using a template cell.
    pub fn clear_line_from_with(&mut self, row: usize, col: usize, template: &Cell) {
        if let Some(row_cells) = self.rows.get_mut(row) {
            let blank = Cell {
                ch: ' ',
                ..*template
            };
            for cell in row_cells.iter_mut().skip(col) {
                *cell = blank;
            }
            self.bump_generation();
        }
    }

    /// Clear from the start of line to the given column (inclusive).
    pub fn clear_line_to(&mut self, row: usize, col: usize) {
        if let Some(row_cells) = self.rows.get_mut(row) {
            for cell in row_cells.iter_mut().take(col + 1) {
                cell.clear();
            }
            self.bump_generation();
        }
    }

    /// Clear from the start of line to the given column (inclusive), using a template cell.
    pub fn clear_line_to_with(&mut self, row: usize, col: usize, template: &Cell) {
        if let Some(row_cells) = self.rows.get_mut(row) {
            let blank = Cell {
                ch: ' ',
                ..*template
            };
            for cell in row_cells.iter_mut().take(col + 1) {
                *cell = blank;
            }
            self.bump_generation();
        }
    }

    /// Clear an entire line.
    pub fn clear_line(&mut self, row: usize) {
        if let Some(row_cells) = self.rows.get_mut(row) {
            for cell in row_cells {
                cell.clear();
            }
            self.bump_generation();
        }
    }

    /// Clear an entire line, using a template cell.
    pub fn clear_line_with(&mut self, row: usize, template: &Cell) {
        if let Some(row_cells) = self.rows.get_mut(row) {
            let blank = Cell {
                ch: ' ',
                ..*template
            };
            for cell in row_cells {
                *cell = blank;
            }
            self.bump_generation();
        }
    }

    /// Clear from (start_row, start_col) to end of screen.
    pub fn clear_screen_from(&mut self, start_row: usize, start_col: usize) {
        self.clear_line_from(start_row, start_col);
        for row in (start_row + 1)..self.height {
            self.clear_line(row);
        }
    }

    /// Clear from (start_row, start_col) to end of screen, using a template cell.
    pub fn clear_screen_from_with(&mut self, start_row: usize, start_col: usize, template: &Cell) {
        self.clear_line_from_with(start_row, start_col, template);
        for row in (start_row + 1)..self.height {
            self.clear_line_with(row, template);
        }
    }

    /// Clear from start of screen to (end_row, end_col).
    pub fn clear_screen_to(&mut self, end_row: usize, end_col: usize) {
        for row in 0..end_row {
            self.clear_line(row);
        }
        self.clear_line_to(end_row, end_col);
    }

    /// Clear from start of screen to (end_row, end_col), using a template cell.
    pub fn clear_screen_to_with(&mut self, end_row: usize, end_col: usize, template: &Cell) {
        for row in 0..end_row {
            self.clear_line_with(row, template);
        }
        self.clear_line_to_with(end_row, end_col, template);
    }

    /// Scroll the entire buffer up by one line.
    /// The top line goes to scrollback; a blank line appears at the bottom.
    pub fn scroll_up(&mut self) {
        self.scroll_region_up(0, self.height.saturating_sub(1));
    }

    /// Scroll the entire buffer up by one line, using a template cell
    /// for the new blank line (preserves current SGR background).
    pub fn scroll_up_with(&mut self, template: &Cell) {
        self.scroll_region_up_with(0, self.height.saturating_sub(1), template);
    }

    /// Scroll the entire buffer down by one line.
    /// The bottom line is lost; a blank line appears at the top.
    pub fn scroll_down(&mut self) {
        self.scroll_region_down(0, self.height.saturating_sub(1));
    }

    /// Scroll a region [top..=bottom] up by one line.
    /// The line at `top` goes to scrollback; a blank line appears at `bottom`.
    pub fn scroll_region_up(&mut self, top: usize, bottom: usize) {
        if !self.rows.is_empty() && top <= bottom && bottom < self.height {
            let removed = self.rows.remove(top);
            if top == 0 && self.scrollback.len() < self.max_scrollback {
                self.scrollback.push(removed);
            } else if top == 0 && !self.scrollback.is_empty() {
                self.scrollback.remove(0);
                self.scrollback.push(removed);
            }
            // When top > 0, the scrolled-out line is simply discarded (not scrollback).
            self.rows.insert(bottom, vec![Cell::default(); self.width]);
            self.bump_generation();
        }
    }

    /// Scroll a region [top..=bottom] up by one line, using a template cell
    /// for the new blank line (preserves current SGR background).
    pub fn scroll_region_up_with(&mut self, top: usize, bottom: usize, template: &Cell) {
        if !self.rows.is_empty() && top <= bottom && bottom < self.height {
            let removed = self.rows.remove(top);
            if top == 0 && self.scrollback.len() < self.max_scrollback {
                self.scrollback.push(removed);
            } else if top == 0 && !self.scrollback.is_empty() {
                self.scrollback.remove(0);
                self.scrollback.push(removed);
            }
            let blank = Cell {
                ch: ' ',
                ..*template
            };
            self.rows.insert(bottom, vec![blank; self.width]);
            self.bump_generation();
        }
    }

    /// Scroll a region [top..=bottom] down by one line.
    /// The line at `bottom` is lost; a blank line appears at `top`.
    pub fn scroll_region_down(&mut self, top: usize, bottom: usize) {
        if !self.rows.is_empty() && top <= bottom && bottom < self.height {
            self.rows.remove(bottom);
            self.rows.insert(top, vec![Cell::default(); self.width]);
            self.bump_generation();
        }
    }

    /// Scroll a region [top..=bottom] down by one line, using a template cell
    /// for the new blank line (preserves current SGR background).
    pub fn scroll_region_down_with(&mut self, top: usize, bottom: usize, template: &Cell) {
        if !self.rows.is_empty() && top <= bottom && bottom < self.height {
            self.rows.remove(bottom);
            let blank = Cell {
                ch: ' ',
                ..*template
            };
            self.rows.insert(top, vec![blank; self.width]);
            self.bump_generation();
        }
    }

    /// Insert a blank line at `row`, pushing lines downward.
    /// Lines that fall past `bottom` are discarded.
    /// If `bottom` is None, the last line of the buffer is discarded.
    pub fn insert_line(&mut self, row: usize, bottom: Option<usize>) {
        let bottom = bottom.unwrap_or(self.height.saturating_sub(1));
        if row < self.height && bottom < self.height && row <= bottom {
            self.rows.insert(row, vec![Cell::default(); self.width]);
            self.rows.remove(bottom + 1);
            self.bump_generation();
        }
    }

    /// Insert a blank line at `row`, using a template cell for the new line.
    pub fn insert_line_with(&mut self, row: usize, bottom: Option<usize>, template: &Cell) {
        let bottom = bottom.unwrap_or(self.height.saturating_sub(1));
        if row < self.height && bottom < self.height && row <= bottom {
            let blank = Cell {
                ch: ' ',
                ..*template
            };
            self.rows.insert(row, vec![blank; self.width]);
            self.rows.remove(bottom + 1);
            self.bump_generation();
        }
    }

    /// Delete the line at `row`, shifting lines below it upward.
    /// A blank line is inserted at `bottom`.
    /// If `bottom` is None, the bottom of the buffer is used.
    pub fn delete_line(&mut self, row: usize, bottom: Option<usize>) {
        let bottom = bottom.unwrap_or(self.height.saturating_sub(1));
        if row < self.height && bottom < self.height && row <= bottom {
            self.rows.remove(row);
            self.rows.insert(bottom, vec![Cell::default(); self.width]);
            self.bump_generation();
        }
    }

    /// Delete the line at `row`, using a template cell for the new blank line.
    pub fn delete_line_with(&mut self, row: usize, bottom: Option<usize>, template: &Cell) {
        let bottom = bottom.unwrap_or(self.height.saturating_sub(1));
        if row < self.height && bottom < self.height && row <= bottom {
            self.rows.remove(row);
            let blank = Cell {
                ch: ' ',
                ..*template
            };
            self.rows.insert(bottom, vec![blank; self.width]);
            self.bump_generation();
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
            self.bump_generation();
        }
    }

    /// Insert blank cells at (row, col), shifting existing cells right.
    /// The new blank cells use the template's attributes.
    pub fn insert_cells_with(&mut self, row: usize, col: usize, count: usize, template: &Cell) {
        if let Some(row_cells) = self.rows.get_mut(row) {
            let count = count.min(self.width - col);
            for i in (col + count..self.width).rev() {
                row_cells[i] = row_cells[i - count];
            }
            let blank = Cell {
                ch: ' ',
                ..*template
            };
            for cell in row_cells.iter_mut().skip(col).take(count) {
                *cell = blank;
            }
            self.bump_generation();
        }
    }

    pub fn delete_cells(&mut self, row: usize, col: usize, count: usize) {
        if let Some(row_cells) = self.rows.get_mut(row) {
            let count = count.min(self.width - col);
            for i in col..(self.width - count) {
                row_cells[i] = row_cells[i + count];
            }
            for cell in row_cells
                .iter_mut()
                .take(self.width)
                .skip(self.width - count)
            {
                cell.clear();
            }
            self.bump_generation();
        }
    }

    /// Delete cells at (row, col), shifting remaining cells left.
    /// The new blank cells at the end use the template's attributes.
    pub fn delete_cells_with(&mut self, row: usize, col: usize, count: usize, template: &Cell) {
        if let Some(row_cells) = self.rows.get_mut(row) {
            let count = count.min(self.width - col);
            for i in col..(self.width - count) {
                row_cells[i] = row_cells[i + count];
            }
            let blank = Cell {
                ch: ' ',
                ..*template
            };
            for cell in row_cells
                .iter_mut()
                .take(self.width)
                .skip(self.width - count)
            {
                *cell = blank;
            }
            self.bump_generation();
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
    /// Display width of the character: 0 (continuation of wide char), 1, or 2.
    /// Required by the client to distinguish normal empty cells (width=1, ch=' ')
    /// from wide-char continuations (width=0) when rendering incremental diffs.
    pub width: u8,
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
                        width: c.width,
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
                                width: a.width,
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
        assert_eq!(b.generation(), 0);
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
        b.insert_line(1, None);
        assert_eq!(b.rows[1][0].ch, ' ');
        assert_eq!(b.rows[2][0].ch, 'B');
        b.delete_line(1, None);
        assert_eq!(b.rows[1][0].ch, 'B');
    }

    #[test]
    fn test_buffer_scroll_region() {
        let mut b = Buffer::new(10, 5, 100);
        // Mark each row with its index
        for i in 0..5 {
            b.rows[i][0].ch = char::from_digit(i as u32, 10).unwrap();
        }
        // Scroll only rows 1-3 (leave rows 0 and 4 untouched)
        b.scroll_region_up(1, 3);
        // Row 0 (char '0') unchanged
        assert_eq!(b.rows[0][0].ch, '0');
        // Row 1 should now have what was row 2 ('2')
        assert_eq!(b.rows[1][0].ch, '2');
        // Row 2 should now have what was row 3 ('3')
        assert_eq!(b.rows[2][0].ch, '3');
        // Row 3 should be blank (scrolled in)
        assert_eq!(b.rows[3][0].ch, ' ');
        // Row 4 (char '4') unchanged
        assert_eq!(b.rows[4][0].ch, '4');
    }

    #[test]
    fn test_buffer_insert_delete_line_with_region() {
        let mut b = Buffer::new(10, 6, 100);
        for i in 0..6 {
            b.rows[i][0].ch = char::from_digit(i as u32, 10).unwrap();
        }
        // Insert at row 2, scroll region 2-4
        b.insert_line(2, Some(4));
        assert_eq!(b.rows[0][0].ch, '0'); // unchanged
        assert_eq!(b.rows[1][0].ch, '1'); // unchanged
        assert_eq!(b.rows[2][0].ch, ' '); // new blank
        assert_eq!(b.rows[3][0].ch, '2'); // shifted from 2
        assert_eq!(b.rows[4][0].ch, '3'); // shifted from 3, '4' discarded
        assert_eq!(b.rows[5][0].ch, '5'); // unchanged (outside region)
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
    fn test_buffer_clear_with_attrs() {
        let mut b = Buffer::new(10, 5, 100);
        b.rows[0][0].ch = 'X';
        b.rows[0][0].fg = [255, 0, 0];
        let template = Cell {
            ch: ' ',
            fg: [0, 128, 255],
            bg: [40, 40, 40],
            ..Default::default()
        };
        b.clear_all_with(&template);
        assert_eq!(b.rows[0][0].ch, ' ');
        assert_eq!(b.rows[0][0].fg, [0, 128, 255]);
        assert_eq!(b.rows[0][0].bg, [40, 40, 40]);
        assert_eq!(b.rows[4][9].bg, [40, 40, 40]);
    }

    #[test]
    fn test_buffer_clear_line_from_with() {
        let mut b = Buffer::new(10, 3, 100);
        b.rows[1][3].ch = 'A';
        b.rows[1][7].ch = 'B';
        let template = Cell {
            ch: ' ',
            bg: [10, 20, 30],
            ..Default::default()
        };
        b.clear_line_from_with(1, 5, &template);
        assert_eq!(b.rows[1][3].ch, 'A'); // before col 5: untouched
        assert_eq!(b.rows[1][5].ch, ' '); // cleared
        assert_eq!(b.rows[1][5].bg, [10, 20, 30]); // template bg
        assert_eq!(b.rows[1][7].ch, ' '); // cleared
        assert_eq!(b.rows[1][7].bg, [10, 20, 30]); // template bg
    }

    #[test]
    fn test_buffer_total_lines() {
        let mut b = Buffer::new(10, 5, 100);
        b.scroll_up();
        b.scroll_up();
        assert_eq!(b.total_lines(), 7);
    }

    // -- Generation counter tests --

    #[test]
    fn test_generation_increments_on_set() {
        let mut b = Buffer::new(10, 5, 100);
        let gen0 = b.generation();
        b.set(0, 0, Cell::new('X'));
        assert_eq!(b.generation(), gen0 + 1);
    }

    #[test]
    fn test_generation_increments_on_clear() {
        let mut b = Buffer::new(10, 5, 100);
        let gen0 = b.generation();
        b.clear_all();
        assert!(b.generation() > gen0);
    }

    #[test]
    fn test_generation_increments_on_scroll() {
        let mut b = Buffer::new(10, 5, 100);
        let gen0 = b.generation();
        b.scroll_up();
        assert!(b.generation() > gen0);
        let gen1 = b.generation();
        b.scroll_region_down(0, 4);
        assert!(b.generation() > gen1);
    }

    #[test]
    fn test_generation_increments_on_insert_delete_cells() {
        let mut b = Buffer::new(10, 5, 100);
        let gen0 = b.generation();
        b.insert_cells(0, 0, 3);
        assert!(b.generation() > gen0);
        let gen1 = b.generation();
        b.delete_cells(0, 0, 2);
        assert!(b.generation() > gen1);
    }

    #[test]
    fn test_generation_increments_on_resize() {
        let mut b = Buffer::new(10, 5, 100);
        let gen0 = b.generation();
        b.resize(20, 10);
        assert!(b.generation() > gen0);
    }

    #[test]
    fn test_generation_does_not_change_on_read() {
        let b = Buffer::new(10, 5, 100);
        let gen = b.generation();
        let _ = b.get(0, 0);
        let _ = b.total_lines();
        let _ = b.get_line(0);
        let _ = b.get_lines(0, 1);
        assert_eq!(b.generation(), gen);
    }

    #[test]
    fn test_generation_wraps_on_overflow() {
        let mut b = Buffer::new(10, 5, 100);
        // Set generation to near u64::MAX
        b.generation = u64::MAX - 1;
        b.set(0, 0, Cell::new('X'));
        assert_eq!(b.generation(), u64::MAX);
        b.set(0, 1, Cell::new('Y'));
        assert_eq!(b.generation(), 0); // wrapped
    }

    // ─── CellDiff width field tests ───

    #[test]
    fn test_diff_includes_width_for_wide_char() {
        let a = Buffer::new(5, 1, 100);
        let mut b_buf = Buffer::new(5, 1, 100);
        // Change cell at col 1 to a wide char
        b_buf.rows[0][1].ch = '你';
        b_buf.rows[0][1].width = 2;
        b_buf.rows[0][2].ch = ' '; // continuation
        b_buf.rows[0][2].width = 0;
        let diff = b_buf.diff(&a);
        // Find the cell at (0, 1) in the diff
        let wide_cell = diff.cells.iter().find(|c| c.row == 0 && c.col == 1);
        assert!(wide_cell.is_some(), "wide char cell should be in diff");
        let wc = wide_cell.unwrap();
        assert_eq!(wc.ch, '你');
        assert_eq!(wc.width, 2, "wide char CellDiff should have width=2");
        // Find the continuation cell at (0, 2)
        let cont_cell = diff.cells.iter().find(|c| c.row == 0 && c.col == 2);
        assert!(cont_cell.is_some(), "continuation cell should be in diff");
        let cc = cont_cell.unwrap();
        assert_eq!(cc.width, 0, "continuation CellDiff should have width=0");
    }

    #[test]
    fn test_diff_includes_width_for_normal_cells() {
        let a = Buffer::new(5, 1, 100);
        let mut b_buf = Buffer::new(5, 1, 100);
        // Change cell at col 0 to 'X' (width=1, default)
        b_buf.rows[0][0].ch = 'X';
        let diff = b_buf.diff(&a);
        let cell = diff.cells.iter().find(|c| c.row == 0 && c.col == 0);
        assert!(cell.is_some());
        assert_eq!(cell.unwrap().width, 1, "normal cell CellDiff should have width=1");
    }

    #[test]
    fn test_diff_dimension_change_includes_width() {
        let a = Buffer::new(3, 2, 100);
        let b_buf = Buffer::new(5, 2, 100);
        let diff = b_buf.diff(&a);
        // All cells should be in the diff (dimension change)
        assert_eq!(diff.cells.len(), 5 * 2);
        // Every cell should have width=1 (default)
        for c in &diff.cells {
            assert_eq!(c.width, 1, "all default cells in dimension-change diff should have width=1");
        }
    }

    #[test]
    fn test_diff_serialization_includes_width() {
        // Verify that the width field survives JSON serialization (as sent to client)
        let mut buf = Buffer::new(5, 1, 100);
        buf.rows[0][2].ch = '你';
        buf.rows[0][2].width = 2;
        buf.rows[0][3].width = 0;
        let diff = buf.diff(&Buffer::new(5, 1, 100));
        let json = serde_json::to_string(&diff).unwrap();
        // The JSON should contain "width":2 and "width":0
        assert!(json.contains("\"width\":2"), "JSON should contain width:2 for wide char");
        assert!(json.contains("\"width\":0"), "JSON should contain width:0 for continuation");
    }
}
