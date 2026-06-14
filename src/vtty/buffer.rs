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

/// A single changed cell in a buffer diff.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CellDiff {
    pub row: usize,
    pub col: usize,
    pub cell: Cell,
}

/// Result of diffing two buffers.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BufferDiff {
    pub width: usize,
    pub height: usize,
    pub cells: Vec<CellDiff>,
    pub changed_count: usize,
}

fn blank_cell(template: Option<&Cell>) -> Cell {
    match template {
        Some(t) => Cell {
            ch: ' ',
            ..*t
        },
        None => Cell::default(),
    }
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

    /// Clear the entire buffer.
    /// If `template` is Some, new cells inherit its style; otherwise Cell::default() is used.
    pub fn clear_all(&mut self, template: Option<&Cell>) {
        let blank = blank_cell(template);
        for row in &mut self.rows {
            for cell in row {
                *cell = blank;
            }
        }
        self.bump_generation();
    }

    /// Clear from the given column to end of line.
    pub fn clear_line_from(&mut self, row: usize, col: usize, template: Option<&Cell>) {
        if let Some(row_cells) = self.rows.get_mut(row) {
            let blank = blank_cell(template);
            for cell in row_cells.iter_mut().skip(col) {
                *cell = blank;
            }
            self.bump_generation();
        }
    }

    /// Clear from the start of line to the given column (inclusive).
    pub fn clear_line_to(&mut self, row: usize, col: usize, template: Option<&Cell>) {
        if let Some(row_cells) = self.rows.get_mut(row) {
            let blank = blank_cell(template);
            for cell in row_cells.iter_mut().take(col + 1) {
                *cell = blank;
            }
            self.bump_generation();
        }
    }

    /// Clear an entire line.
    pub fn clear_line(&mut self, row: usize, template: Option<&Cell>) {
        if let Some(row_cells) = self.rows.get_mut(row) {
            let blank = blank_cell(template);
            for cell in row_cells {
                *cell = blank;
            }
            self.bump_generation();
        }
    }

    /// Clear from (start_row, start_col) to end of screen.
    pub fn clear_screen_from(&mut self, start_row: usize, start_col: usize, template: Option<&Cell>) {
        self.clear_line_from(start_row, start_col, template);
        for row in (start_row + 1)..self.height {
            self.clear_line(row, template);
        }
    }

    /// Clear from start of screen to (end_row, end_col).
    pub fn clear_screen_to(&mut self, end_row: usize, end_col: usize, template: Option<&Cell>) {
        for row in 0..end_row {
            self.clear_line(row, template);
        }
        self.clear_line_to(end_row, end_col, template);
    }

    /// Scroll the entire buffer up by one line.
    /// The top line goes to scrollback; a blank line appears at the bottom.
    pub fn scroll_up(&mut self, template: Option<&Cell>) {
        self.scroll_region_up(0, self.height.saturating_sub(1), template);
    }

    /// Scroll a region [top..=bottom] up by one line.
    /// The line at `top` goes to scrollback; a blank line appears at `bottom`.
    pub fn scroll_region_up(&mut self, top: usize, bottom: usize, template: Option<&Cell>) {
        if !self.rows.is_empty() && top <= bottom && bottom < self.height {
            let removed = self.rows.remove(top);
            if top == 0 && self.scrollback.len() < self.max_scrollback {
                self.scrollback.push(removed);
            } else if top == 0 && !self.scrollback.is_empty() {
                self.scrollback.remove(0);
                self.scrollback.push(removed);
            }
            // When top > 0, the scrolled-out line is simply discarded (not scrollback).
            let blank = blank_cell(template);
            self.rows.insert(bottom, vec![blank; self.width]);
            self.bump_generation();
        }
    }

    /// Scroll a region [top..=bottom] down by one line.
    /// The line at `bottom` is lost; a blank line appears at `top`.
    pub fn scroll_region_down(&mut self, top: usize, bottom: usize, template: Option<&Cell>) {
        if !self.rows.is_empty() && top <= bottom && bottom < self.height {
            self.rows.remove(bottom);
            let blank = blank_cell(template);
            self.rows.insert(top, vec![blank; self.width]);
            self.bump_generation();
        }
    }

    /// Insert a blank line at `row`, pushing lines downward.
    /// Lines that fall past `bottom` are discarded.
    /// If `bottom` is None, the last line of the buffer is discarded.
    pub fn insert_line(&mut self, row: usize, bottom: Option<usize>, template: Option<&Cell>) {
        let bottom = bottom.unwrap_or(self.height.saturating_sub(1));
        if row < self.height && bottom < self.height && row <= bottom {
            let blank = blank_cell(template);
            self.rows.insert(row, vec![blank; self.width]);
            self.rows.remove(bottom + 1);
            self.bump_generation();
        }
    }

    /// Delete the line at `row`, shifting lines below it upward.
    /// A blank line is inserted at `bottom`.
    /// If `bottom` is None, the bottom of the buffer is used.
    pub fn delete_line(&mut self, row: usize, bottom: Option<usize>, template: Option<&Cell>) {
        let bottom = bottom.unwrap_or(self.height.saturating_sub(1));
        if row < self.height && bottom < self.height && row <= bottom {
            self.rows.remove(row);
            let blank = blank_cell(template);
            self.rows.insert(bottom, vec![blank; self.width]);
            self.bump_generation();
        }
    }

    /// Insert blank cells at (row, col), shifting existing cells right.
    pub fn insert_cells(&mut self, row: usize, col: usize, count: usize, template: Option<&Cell>) {
        if let Some(row_cells) = self.rows.get_mut(row) {
            let count = count.min(self.width - col);
            for i in (col + count..self.width).rev() {
                row_cells[i] = row_cells[i - count];
            }
            let blank = blank_cell(template);
            for cell in row_cells.iter_mut().skip(col).take(count) {
                *cell = blank;
            }
            self.bump_generation();
        }
    }

    /// Delete cells at (row, col), shifting remaining cells left.
    pub fn delete_cells(&mut self, row: usize, col: usize, count: usize, template: Option<&Cell>) {
        if let Some(row_cells) = self.rows.get_mut(row) {
            let count = count.min(self.width - col);
            for i in col..(self.width - count) {
                row_cells[i] = row_cells[i + count];
            }
            let blank = blank_cell(template);
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
                        cell: c,
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
                                cell: *a,
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
    fn test_buffer_scroll() {
        let mut b = Buffer::new(10, 3, 100);
        b.rows[0][0].ch = 'A';
        b.scroll_up(None);
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
        b.insert_line(1, None, None);
        assert_eq!(b.rows[1][0].ch, ' ');
        assert_eq!(b.rows[2][0].ch, 'B');
        b.delete_line(1, None, None);
        assert_eq!(b.rows[1][0].ch, 'B');
    }

    #[test]
    fn test_buffer_insert_delete_line_with_region() {
        let mut b = Buffer::new(10, 6, 100);
        for i in 0..6 {
            b.rows[i][0].ch = char::from_digit(i as u32, 10).unwrap();
        }
        // Insert at row 2, scroll region 2-4
        b.insert_line(2, Some(4), None);
        assert_eq!(b.rows[0][0].ch, '0'); // unchanged
        assert_eq!(b.rows[1][0].ch, '1'); // unchanged
        assert_eq!(b.rows[2][0].ch, ' '); // new blank
        assert_eq!(b.rows[3][0].ch, '2'); // shifted from 2
        assert_eq!(b.rows[4][0].ch, '3'); // shifted from 3, '4' discarded
        assert_eq!(b.rows[5][0].ch, '5'); // unchanged (outside region)
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

    // ─── CellDiff tests ───

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
        let wc = &wide_cell.unwrap().cell;
        assert_eq!(wc.ch, '你');
        assert_eq!(wc.width, 2, "wide char CellDiff should have width=2");
        // Find the continuation cell at (0, 2)
        let cont_cell = diff.cells.iter().find(|c| c.row == 0 && c.col == 2);
        assert!(cont_cell.is_some(), "continuation cell should be in diff");
        let cc = &cont_cell.unwrap().cell;
        assert_eq!(cc.width, 0, "continuation CellDiff should have width=0");
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
        // The JSON should contain "width":2 and "width":0 (nested inside cell object)
        assert!(json.contains(r#""width":2"#), "JSON should contain width:2 for wide char");
        assert!(json.contains(r#""width":0"#), "JSON should contain width:0 for continuation");
    }

    #[test]
    fn test_buffer_get_line_scrollback() {
        let mut b = Buffer::new(10, 3, 100);
        b.rows[0][0].ch = 'A';
        b.scroll_up(None);
        // Now scrollback has 1 entry, rows are shifted
        assert_eq!(b.get_line(0).unwrap()[0].ch, 'A'); // scrollback[0]
        assert_eq!(b.get_line(1).unwrap()[0].ch, ' '); // rows[0]
    }

    #[test]
    fn test_buffer_scroll_region_up_with_template() {
        let mut b = Buffer::new(10, 5, 100);
        for i in 0..5 { b.rows[i][0].ch = char::from_digit(i as u32, 10).unwrap(); }
        let tmpl = Cell { fg: [55, 55, 55], ..Default::default() };
        b.scroll_region_up(1, 3, Some(&tmpl));
        assert_eq!(b.rows[0][0].ch, '0'); // unchanged
        assert_eq!(b.rows[1][0].ch, '2');
        assert_eq!(b.rows[2][0].ch, '3');
        assert_eq!(b.rows[3][0].ch, ' ');
        assert_eq!(b.rows[3][0].fg, [55, 55, 55]); // template
        assert_eq!(b.rows[4][0].ch, '4'); // unchanged
    }

    #[test]
    fn test_buffer_scroll_region_down() {
        let mut b = Buffer::new(10, 5, 100);
        for i in 0..5 { b.rows[i][0].ch = char::from_digit(i as u32, 10).unwrap(); }
        b.scroll_region_down(1, 3, None);
        // Row 1 becomes blank
        assert_eq!(b.rows[1][0].ch, ' ');
        assert_eq!(b.rows[0][0].ch, '0'); // unchanged
        assert_eq!(b.rows[2][0].ch, '1'); // shifted from row 1
        assert_eq!(b.rows[3][0].ch, '2'); // shifted from row 2, '3' lost
        assert_eq!(b.rows[4][0].ch, '4'); // unchanged
    }

    #[test]
    fn test_buffer_diff_no_change() {
        let a = Buffer::new(10, 5, 100);
        let b_buf = Buffer::new(10, 5, 100);
        let diff = a.diff(&b_buf);
        assert_eq!(diff.changed_count, 0);
        assert!(diff.cells.is_empty());
    }

    #[test]
    fn test_buffer_diff_cell_changes() {
        let a = Buffer::new(5, 2, 100);
        let mut b_buf = Buffer::new(5, 2, 100);
        b_buf.rows[0][1].ch = 'X';
        b_buf.rows[1][3].ch = 'Y';
        let diff = b_buf.diff(&a);
        assert_eq!(diff.changed_count, 2);
        assert_eq!(diff.cells.len(), 2);
    }

    #[test]
    fn test_buffer_diff_uses_self_not_other_for_changed_cells() {
        let a = Buffer::new(5, 1, 100);
        let mut b_buf = Buffer::new(5, 1, 100);
        b_buf.rows[0][0].ch = 'X';
        b_buf.rows[0][0].fg = [255, 0, 0];
        let diff = b_buf.diff(&a);
        let cell = diff.cells.iter().find(|c| c.col == 0).unwrap();
        assert_eq!(cell.cell.ch, 'X');
        assert_eq!(cell.cell.fg, [255, 0, 0]); // from self (b_buf), not other
    }

    #[test]
    fn test_buffer_delete_line_with_template() {
        let mut b = Buffer::new(10, 5, 100);
        for i in 0..5 { b.rows[i][0].ch = char::from_digit(i as u32, 10).unwrap(); }
        let tmpl = Cell { bg: [11, 22, 33], ..Default::default() };
        b.delete_line(1, None, Some(&tmpl));
        assert_eq!(b.rows[0][0].ch, '0');
        assert_eq!(b.rows[1][0].ch, '2'); // shifted up
        assert_eq!(b.rows[2][0].ch, '3');
        assert_eq!(b.rows[3][0].ch, '4');
        assert_eq!(b.rows[4][0].ch, ' ');
        assert_eq!(b.rows[4][0].bg, [11, 22, 33]); // template blank
    }

    #[test]
    fn test_buffer_insert_line_with_template() {
        let mut b = Buffer::new(10, 5, 100);
        for i in 0..5 { b.rows[i][0].ch = char::from_digit(i as u32, 10).unwrap(); }
        let tmpl = Cell { fg: [123, 0, 0], ..Default::default() };
        b.insert_line(2, None, Some(&tmpl));
        assert_eq!(b.rows[0][0].ch, '0');
        assert_eq!(b.rows[1][0].ch, '1');
        assert_eq!(b.rows[2][0].ch, ' ');
        assert_eq!(b.rows[2][0].fg, [123, 0, 0]); // template
        assert_eq!(b.rows[3][0].ch, '2');
    }
}