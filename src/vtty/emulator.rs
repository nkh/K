use super::{
    buffer::Buffer,
    cell::Cell,
    color::color_256_to_rgb,
    parser::{AnsiParser, AnsiToken},
};
use parking_lot::RwLock;
use std::sync::Arc;

#[derive(Debug, Clone, Copy)]
struct Attributes {
    fg: [u8; 3],
    bg: [u8; 3],
    bold: bool,
    italic: bool,
    underline: bool,
    blink: bool,
    reverse: bool,
    invisible: bool,
    strikethrough: bool,
}

impl Default for Attributes {
    fn default() -> Self {
        Self {
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

impl Attributes {
    fn to_cell(&self, ch: char) -> Cell {
        let (fg, bg) = if self.reverse { (self.bg, self.fg) } else { (self.fg, self.bg) };
        Cell {
            ch, fg, bg,
            bold: self.bold,
            italic: self.italic,
            underline: self.underline,
            blink: self.blink,
            reverse: self.reverse,
            invisible: self.invisible,
            strikethrough: self.strikethrough,
        }
    }

    fn reset(&mut self) {
        *self = Self::default();
    }
}

#[derive(Debug, Clone, Copy)]
struct SavedCursor {
    row: usize,
    col: usize,
    attrs: Attributes,
}

pub struct VttyEmulator {
    buffer: Arc<RwLock<Buffer>>,
    cursor_row: usize,
    cursor_col: usize,
    attrs: Attributes,
    saved_cursor: Option<SavedCursor>,
    parser: AnsiParser,
    cursor_visible: bool,
    alternate_screen: bool,
    main_buffer: Option<Buffer>,
    max_scrollback: usize,
    cols: usize,
    rows: usize,
    auto_wrap: bool,
    insert_mode: bool,
    origin_mode: bool,
    scroll_top: usize,
    scroll_bottom: usize,
}

impl VttyEmulator {
    pub fn new(rows: u16, cols: u16, max_scrollback: usize) -> Self {
        let buffer = Arc::new(RwLock::new(Buffer::new(cols as usize, rows as usize, max_scrollback)));
        let rows = rows as usize;
        let cols = cols as usize;
        Self {
            buffer,
            cursor_row: 0,
            cursor_col: 0,
            attrs: Attributes::default(),
            saved_cursor: None,
            parser: AnsiParser::new(),
            cursor_visible: true,
            alternate_screen: false,
            main_buffer: None,
            max_scrollback,
            cols,
            rows,
            auto_wrap: true,
            insert_mode: false,
            origin_mode: false,
            scroll_top: 0,
            scroll_bottom: rows.saturating_sub(1),
        }
    }

    pub fn feed(&mut self, data: &[u8]) {
        let tokens = self.parser.parse(data);
        for token in tokens {
            self.process_token(token);
        }
    }

    pub fn feed_str(&mut self, s: &str) {
        self.feed(s.as_bytes());
    }

    fn process_token(&mut self, token: AnsiToken) {
        match token {
            AnsiToken::Text(text) => self.write_text(&text),
            AnsiToken::Control(byte) => self.process_control(byte),
            AnsiToken::Csi { params, intermediate, final_byte } => {
                self.process_csi(params, intermediate, final_byte);
            }
            AnsiToken::Osc(_content) => {}
            AnsiToken::Escape(byte) => self.process_escape(byte),
            AnsiToken::Dcs { .. } => {}
        }
    }

    fn write_text(&mut self, text: &str) {
        for ch in text.chars() {
            self.write_char(ch);
        }
    }

    fn write_char(&mut self, ch: char) {
        if ch == '\r' {
            self.cursor_col = 0;
            return;
        }
        if ch == '\n' {
            self.cursor_col = 0;
            self.cursor_row += 1;
            self.check_scroll();
            return;
        }
        if ch == '\t' {
            let next_tab = ((self.cursor_col / 8) + 1) * 8;
            self.cursor_col = next_tab.min(self.cols.saturating_sub(1));
            return;
        }

        let mut buf = self.buffer.write();
        if self.insert_mode {
            buf.insert_cells(self.cursor_row, self.cursor_col, 1);
        }
        if self.cursor_row < self.rows && self.cursor_col < self.cols {
            let cell = self.attrs.to_cell(ch);
            buf.set(self.cursor_row, self.cursor_col, cell);
        }
        self.cursor_col += 1;
        if self.cursor_col >= self.cols {
            if self.auto_wrap {
                self.cursor_col = 0;
                self.cursor_row += 1;
                self.check_scroll_with_buffer(&mut buf);
            } else {
                self.cursor_col = self.cols.saturating_sub(1);
            }
        }
    }

    fn process_control(&mut self, byte: u8) {
        match byte {
            0x07 => {}
            0x08 => {
                if self.cursor_col > 0 {
                    self.cursor_col -= 1;
                }
            }
            0x09 => {
                let next_tab = ((self.cursor_col / 8) + 1) * 8;
                self.cursor_col = next_tab.min(self.cols.saturating_sub(1));
            }
            0x0a | 0x0b | 0x0c => {
                self.cursor_col = 0;
                self.cursor_row += 1;
                self.check_scroll();
            }
            0x0d => { self.cursor_col = 0; }
            _ => {}
        }
    }

    fn process_csi(&mut self, params: Vec<Vec<u16>>, intermediate: Vec<u8>, final_byte: u8) {
        let param = |idx: usize, default: u16| -> u16 {
            params.get(idx)
                .and_then(|p| p.first().copied())
                .unwrap_or(default)
        };
        let param_1based = |idx: usize, default: usize| -> usize {
            (param(idx, default as u16) as usize).saturating_sub(1)
        };

        match final_byte {
            b'H' | b'f' => {
                let row = param_1based(0, 1);
                let col = param_1based(1, 1);
                self.cursor_row = row.min(self.rows.saturating_sub(1));
                self.cursor_col = col.min(self.cols.saturating_sub(1));
            }
            b'A' => {
                let n = param(0, 1) as usize;
                self.cursor_row = self.cursor_row.saturating_sub(n);
            }
            b'B' => {
                let n = param(0, 1) as usize;
                self.cursor_row = (self.cursor_row + n).min(self.rows.saturating_sub(1));
            }
            b'C' => {
                let n = param(0, 1) as usize;
                self.cursor_col = (self.cursor_col + n).min(self.cols.saturating_sub(1));
            }
            b'D' => {
                let n = param(0, 1) as usize;
                self.cursor_col = self.cursor_col.saturating_sub(n);
            }
            b'E' => {
                let n = param(0, 1) as usize;
                self.cursor_row = (self.cursor_row + n).min(self.rows.saturating_sub(1));
                self.cursor_col = 0;
            }
            b'F' => {
                let n = param(0, 1) as usize;
                self.cursor_row = self.cursor_row.saturating_sub(n);
                self.cursor_col = 0;
            }
            b'G' => {
                let col = param_1based(0, 1);
                self.cursor_col = col.min(self.cols.saturating_sub(1));
            }
            b'J' => {
                let mode = param(0, 0);
                let mut buf = self.buffer.write();
                match mode {
                    0 => buf.clear_screen_from(self.cursor_row, self.cursor_col),
                    1 => buf.clear_screen_to(self.cursor_row, self.cursor_col),
                    2 | 3 => buf.clear_all(),
                    _ => {}
                }
            }
            b'K' => {
                let mode = param(0, 0);
                let mut buf = self.buffer.write();
                match mode {
                    0 => buf.clear_line_from(self.cursor_row, self.cursor_col),
                    1 => buf.clear_line_to(self.cursor_row, self.cursor_col),
                    2 => buf.clear_line(self.cursor_row),
                    _ => {}
                }
            }
            b'L' => {
                let n = param(0, 1) as usize;
                let mut buf = self.buffer.write();
                for _ in 0..n { buf.insert_line(self.cursor_row); }
            }
            b'M' => {
                let n = param(0, 1) as usize;
                let mut buf = self.buffer.write();
                for _ in 0..n { buf.delete_line(self.cursor_row); }
            }
            b'P' => {
                let n = param(0, 1) as usize;
                let mut buf = self.buffer.write();
                buf.delete_cells(self.cursor_row, self.cursor_col, n);
            }
            b'@' => {
                let n = param(0, 1) as usize;
                let mut buf = self.buffer.write();
                buf.insert_cells(self.cursor_row, self.cursor_col, n);
            }
            b'S' => {
                let n = param(0, 1) as usize;
                let mut buf = self.buffer.write();
                for _ in 0..n { buf.scroll_up(); }
            }
            b'T' => {
                let n = param(0, 1) as usize;
                let mut buf = self.buffer.write();
                for _ in 0..n { buf.scroll_down(); }
            }
            b'm' => { self.process_sgr(&params); }
            b'h' => {
                if intermediate.first() == Some(&b'?') {
                    self.process_dec_private_mode(&params, true);
                }
            }
            b'l' => {
                if intermediate.first() == Some(&b'?') {
                    self.process_dec_private_mode(&params, false);
                }
            }
            b'r' => {
                let top = param_1based(0, 1);
                let bottom = param_1based(1, self.rows);
                self.scroll_top = top.min(self.rows.saturating_sub(1));
                self.scroll_bottom = bottom.min(self.rows.saturating_sub(1));
            }
            _ => {}
        }
    }

    fn process_sgr(&mut self, params: &[Vec<u16>]) {
        if params.is_empty() {
            self.attrs.reset();
            return;
        }
        let mut i = 0;
        while i < params.len() {
            let param = params[i].first().copied().unwrap_or(0);
            match param {
                0 => self.attrs.reset(),
                1 => self.attrs.bold = true,
                3 => self.attrs.italic = true,
                4 => self.attrs.underline = true,
                5 => self.attrs.blink = true,
                7 => self.attrs.reverse = true,
                8 => self.attrs.invisible = true,
                9 => self.attrs.strikethrough = true,
                21 | 22 => self.attrs.bold = false,
                23 => self.attrs.italic = false,
                24 => self.attrs.underline = false,
                25 => self.attrs.blink = false,
                27 => self.attrs.reverse = false,
                28 => self.attrs.invisible = false,
                29 => self.attrs.strikethrough = false,
                30..=37 => { self.attrs.fg = color_256_to_rgb(param as u8 - 30); }
                38 => {
                    if let Some(next) = params.get(i + 1) {
                        match next.first().copied() {
                            Some(2) => {
                                if i + 4 < params.len() {
                                    self.attrs.fg = [
                                        params[i + 2].first().copied().unwrap_or(0) as u8,
                                        params[i + 3].first().copied().unwrap_or(0) as u8,
                                        params[i + 4].first().copied().unwrap_or(0) as u8,
                                    ];
                                    i += 4;
                                }
                            }
                            Some(5) => {
                                if let Some(color_param) = params.get(i + 2) {
                                    let idx = color_param.first().copied().unwrap_or(0) as u8;
                                    self.attrs.fg = color_256_to_rgb(idx);
                                    i += 2;
                                }
                            }
                            _ => {}
                        }
                    }
                }
                39 => self.attrs.fg = [204, 204, 204],
                40..=47 => { self.attrs.bg = color_256_to_rgb(param as u8 - 40); }
                48 => {
                    if let Some(next) = params.get(i + 1) {
                        match next.first().copied() {
                            Some(2) => {
                                if i + 4 < params.len() {
                                    self.attrs.bg = [
                                        params[i + 2].first().copied().unwrap_or(0) as u8,
                                        params[i + 3].first().copied().unwrap_or(0) as u8,
                                        params[i + 4].first().copied().unwrap_or(0) as u8,
                                    ];
                                    i += 4;
                                }
                            }
                            Some(5) => {
                                if let Some(color_param) = params.get(i + 2) {
                                    let idx = color_param.first().copied().unwrap_or(0) as u8;
                                    self.attrs.bg = color_256_to_rgb(idx);
                                    i += 2;
                                }
                            }
                            _ => {}
                        }
                    }
                }
                49 => self.attrs.bg = [0, 0, 0],
                90..=97 => { self.attrs.fg = color_256_to_rgb(param as u8 - 90 + 8); }
                100..=107 => { self.attrs.bg = color_256_to_rgb(param as u8 - 100 + 8); }
                _ => {}
            }
            i += 1;
        }
    }

    fn process_dec_private_mode(&mut self, params: &[Vec<u16>], set: bool) {
        for param in params {
            let mode = param.first().copied().unwrap_or(0);
            match mode {
                25 => self.cursor_visible = set,
                47 | 1047 | 1049 => {
                    if set && !self.alternate_screen {
                        self.enter_alternate_screen();
                    } else if !set && self.alternate_screen {
                        self.exit_alternate_screen();
                    }
                }
                7 => self.auto_wrap = set,
                6 => self.origin_mode = set,
                _ => {}
            }
        }
    }

    fn process_escape(&mut self, byte: u8) {
        match byte {
            b'7' => {
                self.saved_cursor = Some(SavedCursor {
                    row: self.cursor_row,
                    col: self.cursor_col,
                    attrs: self.attrs,
                });
            }
            b'8' => {
                if let Some(saved) = self.saved_cursor {
                    self.cursor_row = saved.row.min(self.rows.saturating_sub(1));
                    self.cursor_col = saved.col.min(self.cols.saturating_sub(1));
                    self.attrs = saved.attrs;
                }
            }
            b'M' => {
                if self.cursor_row == 0 {
                    let mut buf = self.buffer.write();
                    buf.scroll_down();
                } else {
                    self.cursor_row -= 1;
                }
            }
            b'c' => { self.full_reset(); }
            _ => {}
        }
    }

    fn check_scroll(&mut self) {
        if self.cursor_row > self.scroll_bottom {
            let mut buf = self.buffer.write();
            self.check_scroll_with_buffer(&mut buf);
        }
    }

    fn check_scroll_with_buffer(&mut self, buf: &mut Buffer) {
        while self.cursor_row > self.scroll_bottom {
            buf.scroll_up();
            self.cursor_row -= 1;
        }
    }

    fn enter_alternate_screen(&mut self) {
        self.alternate_screen = true;
        self.saved_cursor = Some(SavedCursor {
            row: self.cursor_row,
            col: self.cursor_col,
            attrs: self.attrs,
        });
        let mut buf = self.buffer.write();
        let old = std::mem::replace(&mut *buf, Buffer::new(self.cols, self.rows, self.max_scrollback));
        self.main_buffer = Some(old);
        self.cursor_row = 0;
        self.cursor_col = 0;
    }

    fn exit_alternate_screen(&mut self) {
        self.alternate_screen = false;
        if let Some(main) = self.main_buffer.take() {
            let mut buf = self.buffer.write();
            *buf = main;
        }
        if let Some(saved) = self.saved_cursor {
            self.cursor_row = saved.row;
            self.cursor_col = saved.col;
            self.attrs = saved.attrs;
        }
    }

    fn full_reset(&mut self) {
        self.cursor_row = 0;
        self.cursor_col = 0;
        self.attrs.reset();
        self.saved_cursor = None;
        self.cursor_visible = true;
        self.auto_wrap = true;
        self.insert_mode = false;
        self.origin_mode = false;
        self.scroll_top = 0;
        self.scroll_bottom = self.rows.saturating_sub(1);
        {
            let mut buf = self.buffer.write();
            buf.clear_all();
        }
        if self.alternate_screen {
            self.exit_alternate_screen();
        }
    }

    // Public API
    pub fn buffer(&self) -> Buffer {
        self.buffer.read().clone()
    }

    pub fn snapshot(&self) -> Buffer {
        self.buffer.read().clone()
    }

    pub fn contents_plain(&self) -> String {
        let buf = self.buffer.read();
        buf.rows.iter()
            .map(|row| row.iter().map(|c| c.ch).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn contents_ansi(&self) -> String {
        let buf = self.buffer.read();
        let mut output = String::new();
        let mut last_fg: Option<[u8; 3]> = None;
        let mut last_bg: Option<[u8; 3]> = None;
        let mut last_bold = false;
        let mut last_italic = false;
        let mut last_underline = false;

        for row in &buf.rows {
            for cell in row {
                let mut codes = Vec::new();
                if cell.bold != last_bold {
                    if cell.bold { codes.push("1".to_string()); }
                    else { codes.push("22".to_string()); }
                    last_bold = cell.bold;
                }
                if cell.italic != last_italic {
                    if cell.italic { codes.push("3".to_string()); }
                    else { codes.push("23".to_string()); }
                    last_italic = cell.italic;
                }
                if cell.underline != last_underline {
                    if cell.underline { codes.push("4".to_string()); }
                    else { codes.push("24".to_string()); }
                    last_underline = cell.underline;
                }
                if Some(cell.fg) != last_fg {
                    codes.push(format!("38;2;{};{};{}", cell.fg[0], cell.fg[1], cell.fg[2]));
                    last_fg = Some(cell.fg);
                }
                if Some(cell.bg) != last_bg {
                    codes.push(format!("48;2;{};{};{}", cell.bg[0], cell.bg[1], cell.bg[2]));
                    last_bg = Some(cell.bg);
                }
                if !codes.is_empty() {
                    output.push_str(&format!("\x1b[{}m", codes.join(";")));
                }
                output.push(cell.ch);
            }
            output.push('\n');
        }
        output.push_str("\x1b[0m");
        output
    }

    pub fn partial(&self, start_row: usize, row_count: usize) -> String {
        let buf = self.buffer.read();
        let start = start_row.min(buf.rows.len());
        let end = (start + row_count).min(buf.rows.len());
        buf.rows[start..end].iter()
            .map(|row| row.iter().map(|c| c.ch).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn cursor(&self) -> (usize, usize) {
        (self.cursor_row, self.cursor_col)
    }

    pub fn dimensions(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }

    pub fn resize(&mut self, new_rows: usize, new_cols: usize) {
        {
            let mut buf = self.buffer.write();
            buf.resize(new_cols, new_rows);
        }
        self.rows = new_rows;
        self.cols = new_cols;
        self.cursor_row = self.cursor_row.min(new_rows.saturating_sub(1));
        self.cursor_col = self.cursor_col.min(new_cols.saturating_sub(1));
        self.scroll_bottom = new_rows.saturating_sub(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_text() {
        let mut emu = VttyEmulator::new(10, 10, 100);
        emu.feed_str("Hello");
        assert_eq!(emu.cursor(), (0, 5));
        let buf = emu.buffer();
        assert_eq!(buf.rows[0][0].ch, 'H');
        assert_eq!(buf.rows[0][4].ch, 'o');
    }

    #[test]
    fn test_newline() {
        let mut emu = VttyEmulator::new(10, 10, 100);
        emu.feed_str("Hello\nWorld");
        assert_eq!(emu.cursor(), (1, 5));
        let buf = emu.buffer();
        assert_eq!(buf.rows[1][0].ch, 'W');
    }

    #[test]
    fn test_cursor_movement() {
        let mut emu = VttyEmulator::new(10, 10, 100);
        emu.feed_str("Hello\x1b[2;3H");
        assert_eq!(emu.cursor(), (1, 2));
    }

    #[test]
    fn test_colors() {
        let mut emu = VttyEmulator::new(10, 10, 100);
        emu.feed_str("\x1b[31mRed\x1b[0mNormal");
        let buf = emu.buffer();
        assert_eq!(buf.rows[0][0].fg, [170, 0, 0]);
        assert_eq!(buf.rows[0][3].fg, [204, 204, 204]);
    }

    #[test]
    fn test_truecolor() {
        let mut emu = VttyEmulator::new(10, 10, 100);
        emu.feed_str("\x1b[38;2;255;128;64mX");
        let buf = emu.buffer();
        assert_eq!(buf.rows[0][0].fg, [255, 128, 64]);
    }

    #[test]
    fn test_erase_display() {
        let mut emu = VttyEmulator::new(10, 10, 100);
        emu.feed_str("Hello\nWorld\nTest");
        emu.feed_str("\x1b[2J");
        let buf = emu.buffer();
        assert_eq!(buf.rows[0][0].ch, ' ');
        assert_eq!(buf.rows[1][0].ch, ' ');
    }

    #[test]
    fn test_scroll() {
        let mut emu = VttyEmulator::new(3, 10, 100);
        emu.feed_str("Line1\nLine2\nLine3\nLine4");
        let buf = emu.buffer();
        assert_eq!(buf.scrollback.len(), 1);
        assert_eq!(buf.rows[0][0].ch, 'L');
        assert_eq!(buf.rows[2][0].ch, 'L');
    }

    #[test]
    fn test_save_restore_cursor() {
        let mut emu = VttyEmulator::new(10, 10, 100);
        emu.feed_str("\x1b[5;5H");
        emu.feed_str("\x1b[31m");
        emu.feed_str("\x1b7");
        emu.feed_str("\x1b[1;1H");
        emu.feed_str("\x1b[32m");
        emu.feed_str("\x1b8");
        assert_eq!(emu.cursor(), (4, 4));
        let buf = emu.buffer();
        assert_eq!(buf.rows[4][4].fg, [170, 0, 0]);
    }

    #[test]
    fn test_alternate_screen() {
        let mut emu = VttyEmulator::new(10, 10, 100);
        emu.feed_str("Main");
        emu.feed_str("\x1b[?1049h");
        assert!(emu.alternate_screen);
        let buf = emu.buffer();
        assert_eq!(buf.rows[0][0].ch, ' ');
        emu.feed_str("Alt");
        emu.feed_str("\x1b[?1049l");
        assert!(!emu.alternate_screen);
        let buf2 = emu.buffer();
        assert_eq!(buf2.rows[0][0].ch, 'M');
        assert_eq!(buf2.rows[0][1].ch, 'a');
    }

    #[test]
    fn test_insert_delete_line() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("\x1b[2;1HLine2");
        emu.feed_str("\x1b[2;1H\x1b[L");
        let buf = emu.buffer();
        assert_eq!(buf.rows[1][0].ch, ' ');
        assert_eq!(buf.rows[2][0].ch, 'L');
    }

    #[test]
    fn test_contents_plain() {
        let mut emu = VttyEmulator::new(3, 10, 100);
        emu.feed_str("Hello\nWorld");
        let text = emu.contents_plain();
        assert!(text.contains("Hello"));
        assert!(text.contains("World"));
    }

    #[test]
    fn test_contents_ansi() {
        let mut emu = VttyEmulator::new(3, 10, 100);
        emu.feed_str("\x1b[31mRed\x1b[0m");
        let ansi = emu.contents_ansi();
        assert!(ansi.contains("38;2;170;0;0"));
        assert!(ansi.contains("Red"));
    }

    #[test]
    fn test_resize() {
        let mut emu = VttyEmulator::new(10, 10, 100);
        emu.feed_str("Hello");
        emu.resize(5, 5);
        assert_eq!(emu.dimensions(), (5, 5));
        let buf = emu.buffer();
        assert_eq!(buf.rows[0][0].ch, 'H');
    }
}
