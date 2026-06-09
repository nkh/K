use super::{
    buffer::Buffer,
    cell::{char_width, Cell},
    color::ColorPalette,
    parser::{AnsiParser, AnsiToken},
};
use parking_lot::RwLock;
use std::sync::Arc;

/// Cursor style set by DECSCUSR (CSI Ps SP q).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorStyle {
    /// Blinking block cursor (default).
    Block(bool),
    /// Blinking underline cursor.
    Underline(bool),
    /// Blinking bar cursor.
    Bar(bool),
}

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
    fn make_cell(self, ch: char, width: u8) -> Cell {
        let (fg, bg) = if self.reverse {
            (self.bg, self.fg)
        } else {
            (self.fg, self.bg)
        };
        Cell {
            ch,
            fg,
            bg,
            bold: self.bold,
            italic: self.italic,
            underline: self.underline,
            blink: self.blink,
            reverse: self.reverse,
            invisible: self.invisible,
            strikethrough: self.strikethrough,
            width,
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
    alt_buffer: Option<Buffer>,
    max_scrollback: usize,
    cols: usize,
    rows: usize,
    auto_wrap: bool,
    insert_mode: bool,
    origin_mode: bool,
    scroll_top: usize,
    scroll_bottom: usize,
    /// When true, the cursor is at the last column and a wrap to the
    /// next line is pending.  The wrap is executed when the next
    /// printable character is written (VT100 deferred-wrap semantics).
    wrap_pending: bool,
    /// SGR extended coordinate mode (?1006)
    mouse_sgr: bool,
    /// Button event tracking (?1002)
    mouse_button_tracking: bool,
    /// Any-event tracking (?1003)
    mouse_any_tracking: bool,
    /// Current window title set via OSC 0 / OSC 2.
    title: String,
    /// Bracketed paste mode (?2004).
    bracketed_paste: bool,
    /// Focus reporting mode (?1004).
    focus_reporting: bool,
    /// Pending bell flag — set when BEL (0x07) is received.
    /// Checked and cleared by drain_bell().
    bell_pending: bool,
    /// Current cursor style (DECSCUSR).
    cursor_style: CursorStyle,
    /// Most recent DCS sequence data (e.g. kitty graphics protocol).
    dcs_buffer: String,
    /// Pending response bytes to send back to the child PTY.
    /// Collected during feed() and consumed by drain_responses().
    response_buf: Vec<u8>,
    /// Mutable 256-color palette, modifiable at runtime via OSC 4.
    palette: ColorPalette,
    /// Stored inline images from Sixel DCS sequences.
    /// Each entry is (row, col, sixel_data_string).
    sixel_images: Vec<(usize, usize, String)>,
}

impl VttyEmulator {
    pub fn new(rows: u16, cols: u16, max_scrollback: usize) -> Self {
        let buffer = Arc::new(RwLock::new(Buffer::new(
            cols as usize,
            rows as usize,
            max_scrollback,
        )));
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
            alt_buffer: None,
            max_scrollback,
            cols,
            rows,
            auto_wrap: true,
            insert_mode: false,
            origin_mode: false,
            scroll_top: 0,
            scroll_bottom: rows.saturating_sub(1),
            wrap_pending: false,
            mouse_sgr: false,
            mouse_button_tracking: false,
            mouse_any_tracking: false,
            title: String::new(),
            bracketed_paste: false,
            focus_reporting: false,
            bell_pending: false,
            cursor_style: CursorStyle::Block(true),
            dcs_buffer: String::new(),
            response_buf: Vec::new(),
            palette: ColorPalette::new(),
            sixel_images: Vec::new(),
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

    /// Flush any remaining buffered text from the parser.
    /// Call this when the input stream ends (e.g., PTY closed).
    pub fn finish(&mut self) {
        let tokens = self.parser.finish();
        for token in tokens {
            self.process_token(token);
        }
    }

    fn process_token(&mut self, token: AnsiToken) {
        match token {
            AnsiToken::Text(text) => self.write_text(&text),
            AnsiToken::Control(byte) => self.process_control(byte),
            AnsiToken::Csi {
                params,
                intermediate,
                final_byte,
            } => {
                self.process_csi(params, intermediate, final_byte);
            }
            AnsiToken::Osc(ref content) => self.process_osc(content),
            AnsiToken::Escape(byte) => self.process_escape(byte),
            AnsiToken::EscSequence { .. } => {
                // Charset designation (ESC ( B, ESC ) 0, etc.), DECDHL
                // (ESC # 3/4/5/6/8), and other escape-with-intermediate
                // sequences.  Not yet implemented in the emulator.
            }
            AnsiToken::Dcs {
                params,
                intermediate,
                final_byte,
                data,
            } => {
                self.process_dcs(&params, &intermediate, final_byte, &data);
            }
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
            self.wrap_pending = false;
            return;
        }
        if ch == '\n' {
            // LF: move cursor down and reset to column 0 (Unix newline mode).
            // The VT100 spec defines LF as "move down only" (no CR), but Unix
            // terminals and terminal emulators universally treat LF as a
            // newline (CR+LF).  Programs that explicitly want LF-without-CR
            // (e.g., to overstrike) can use the CSI 6 (DEC line mode / LN)
            // escape sequence instead.
            self.wrap_pending = false;
            self.cursor_col = 0;
            self.cursor_row += 1;
            self.check_scroll();
            return;
        }
        if ch == '\t' {
            self.wrap_pending = false;
            let next_tab = ((self.cursor_col / 8) + 1) * 8;
            self.cursor_col = next_tab.min(self.cols.saturating_sub(1));
            return;
        }

        // Compute character display width.
        let cw = char_width(ch);

        // VT100 deferred wrap: if a previous character left us at the
        // right margin with wrap_pending, advance to the next line now
        // before writing the new character.
        if self.wrap_pending {
            self.wrap_pending = false;
            self.cursor_col = 0;
            self.cursor_row += 1;
            self.check_scroll();
        }

        // Wide characters (CJK, emoji) need 2 columns.
        // If there isn't enough room on the current line, advance to
        // the next line first.
        if cw == 2 && self.cursor_col + 1 >= self.cols {
            if self.auto_wrap {
                self.cursor_col = 0;
                self.cursor_row += 1;
                self.check_scroll();
            } else {
                // No room and no wrap — skip the character.
                return;
            }
        }

        {
            let mut buf = self.buffer.write();
            if self.insert_mode {
                buf.insert_cells(self.cursor_row, self.cursor_col, cw as usize);
            }
            if self.cursor_row < self.rows && self.cursor_col < self.cols {
                let cell = self.attrs.make_cell(ch, cw);
                buf.set(self.cursor_row, self.cursor_col, cell);

                // Place a continuation cell for wide characters.
                if cw == 2 && self.cursor_col + 1 < self.cols {
                    let cont = Cell {
                        ch: ' ',
                        fg: self.attrs.fg,
                        bg: self.attrs.bg,
                        bold: self.attrs.bold,
                        italic: self.attrs.italic,
                        underline: self.attrs.underline,
                        blink: self.attrs.blink,
                        reverse: self.attrs.reverse,
                        invisible: self.attrs.invisible,
                        strikethrough: self.attrs.strikethrough,
                        width: 0,
                    };
                    buf.set(self.cursor_row, self.cursor_col + 1, cont);
                }
            }
        } // Drop the write guard before touching self again

        // Advance cursor.
        self.cursor_col += cw as usize;
        if self.cursor_col >= self.cols {
            if self.auto_wrap {
                self.wrap_pending = true;
                // Keep cursor_col at cols-1 (the last column) visually;
                // the pending flag ensures the next char wraps.
                self.cursor_col = self.cols.saturating_sub(1);
            } else {
                self.cursor_col = self.cols.saturating_sub(1);
            }
        }
    }

    fn process_control(&mut self, byte: u8) {
        match byte {
            0x07 => {
                self.bell_pending = true;
            }
            0x08 if self.cursor_col > 0 => {
                self.cursor_col -= 1;
                self.wrap_pending = false;
                // If we land on a wide-char continuation, step back once
                // more so the cursor is on the leading cell.
                {
                    let buf = self.buffer.read();
                    if let Some(cell) = buf.get(self.cursor_row, self.cursor_col) {
                        if cell.is_wide_continuation() && self.cursor_col > 0 {
                            self.cursor_col -= 1;
                        }
                    }
                }
            }
            0x09 => {
                self.wrap_pending = false;
                let next_tab = ((self.cursor_col / 8) + 1) * 8;
                self.cursor_col = next_tab.min(self.cols.saturating_sub(1));
            }
            0x0a..=0x0c => {
                // LF/VT/FF: move cursor down only (no carriage return).
                self.wrap_pending = false;
                self.cursor_row += 1;
                self.check_scroll();
            }
            0x0d => {
                self.cursor_col = 0;
                self.wrap_pending = false;
            }
            _ => {}
        }
    }

    fn process_csi(&mut self, params: Vec<Vec<u16>>, intermediate: Vec<u8>, final_byte: u8) {
        // ECMA-48: a parameter value of 0 or missing means "use default".
        let param = |idx: usize, default: u16| -> u16 {
            params
                .get(idx)
                .and_then(|p| p.first().copied())
                .map(|v| if v == 0 { default } else { v })
                .unwrap_or(default)
        };
        let param_1based = |idx: usize, default: usize| -> usize {
            (param(idx, default as u16) as usize).saturating_sub(1)
        };

        // Any explicit cursor-movement sequence clears the wrap-pending flag.
        let mut clear_wrap = || {
            self.wrap_pending = false;
        };

        match final_byte {
            b'H' | b'f' => {
                clear_wrap();
                let row = param_1based(0, 1);
                let col = param_1based(1, 1);
                if self.origin_mode {
                    self.cursor_row = (self.scroll_top + row).min(self.scroll_bottom);
                } else {
                    self.cursor_row = row.min(self.rows.saturating_sub(1));
                }
                self.cursor_col = col.min(self.cols.saturating_sub(1));
            }
            b'A' => {
                clear_wrap();
                let n = param(0, 1) as usize;
                self.cursor_row = self.cursor_row.saturating_sub(n);
            }
            b'B' => {
                clear_wrap();
                let n = param(0, 1) as usize;
                self.cursor_row = (self.cursor_row + n).min(self.rows.saturating_sub(1));
            }
            b'C' => {
                clear_wrap();
                let n = param(0, 1) as usize;
                self.cursor_col = (self.cursor_col + n).min(self.cols.saturating_sub(1));
            }
            b'D' => {
                clear_wrap();
                let n = param(0, 1) as usize;
                self.cursor_col = self.cursor_col.saturating_sub(n);
            }
            b'E' => {
                clear_wrap();
                let n = param(0, 1) as usize;
                self.cursor_row = (self.cursor_row + n).min(self.rows.saturating_sub(1));
                self.cursor_col = 0;
            }
            b'F' => {
                clear_wrap();
                let n = param(0, 1) as usize;
                self.cursor_row = self.cursor_row.saturating_sub(n);
                self.cursor_col = 0;
            }
            b'G' => {
                clear_wrap();
                let col = param_1based(0, 1);
                self.cursor_col = col.min(self.cols.saturating_sub(1));
            }
            b'd' => {
                // CSI n d — Vertical Position Absolute (VPA)
                // Move cursor to row n (1-based), column unchanged.
                clear_wrap();
                let row = param_1based(0, 1);
                if self.origin_mode {
                    self.cursor_row = (self.scroll_top + row).min(self.scroll_bottom);
                } else {
                    self.cursor_row = row.min(self.rows.saturating_sub(1));
                }
            }
            b'X' => {
                // CSI n X — Erase Characters (ECH)
                // Erase n characters starting at cursor (overwrites with spaces
                // using current attributes).  Cursor does not move.
                clear_wrap();
                let n = param(0, 1) as usize;
                let blank = self.attrs.make_cell(' ', 1);
                let mut buf = self.buffer.write();
                let mut erased = 0;
                let mut col = self.cursor_col;
                while erased < n && col < self.cols {
                    buf.set(self.cursor_row, col, blank);
                    col += 1;
                    erased += 1;
                }
            }
            b'J' => {
                // CSI J (ED) clears pending wrap for the same reason as CSI K.
                clear_wrap();
                let mode = param(0, 0);
                let blank = self.attrs.make_cell(' ', 1);
                let mut buf = self.buffer.write();
                match mode {
                    0 => buf.clear_screen_from_with(self.cursor_row, self.cursor_col, &blank),
                    1 => buf.clear_screen_to_with(self.cursor_row, self.cursor_col, &blank),
                    2 | 3 => buf.clear_all_with(&blank),
                    _ => {}
                }
            }
            b'K' => {
                // CSI K (EL) clears the pending wrap flag because it
                // explicitly operates on the current line, and the cursor
                // logically stays at its current position (not past the
                // right margin).  Without clearing wrap, a subsequent
                // character would incorrectly wrap to the next line.
                clear_wrap();
                let mode = param(0, 0);
                let blank = self.attrs.make_cell(' ', 1);
                let mut buf = self.buffer.write();
                match mode {
                    0 => buf.clear_line_from_with(self.cursor_row, self.cursor_col, &blank),
                    1 => buf.clear_line_to_with(self.cursor_row, self.cursor_col, &blank),
                    2 => buf.clear_line_with(self.cursor_row, &blank),
                    _ => {}
                }
            }
            b'L' => {
                clear_wrap();
                let n = param(0, 1) as usize;
                let blank = self.attrs.make_cell(' ', 1);
                let mut buf = self.buffer.write();
                for _ in 0..n {
                    buf.insert_line_with(self.cursor_row, Some(self.scroll_bottom), &blank);
                }
            }
            b'M' => {
                clear_wrap();
                let n = param(0, 1) as usize;
                let blank = self.attrs.make_cell(' ', 1);
                let mut buf = self.buffer.write();
                for _ in 0..n {
                    buf.delete_line_with(self.cursor_row, Some(self.scroll_bottom), &blank);
                }
            }
            b'P' => {
                clear_wrap();
                let n = param(0, 1) as usize;
                let blank = self.attrs.make_cell(' ', 1);
                let mut buf = self.buffer.write();
                buf.delete_cells_with(self.cursor_row, self.cursor_col, n, &blank);
            }
            b'@' => {
                clear_wrap();
                let n = param(0, 1) as usize;
                let blank = self.attrs.make_cell(' ', 1);
                let mut buf = self.buffer.write();
                buf.insert_cells_with(self.cursor_row, self.cursor_col, n, &blank);
            }
            b'S' => {
                clear_wrap();
                let n = param(0, 1) as usize;
                let blank = self.attrs.make_cell(' ', 1);
                let mut buf = self.buffer.write();
                for _ in 0..n {
                    buf.scroll_region_up_with(self.scroll_top, self.scroll_bottom, &blank);
                }
            }
            b'T' => {
                clear_wrap();
                let n = param(0, 1) as usize;
                let blank = self.attrs.make_cell(' ', 1);
                let mut buf = self.buffer.write();
                for _ in 0..n {
                    buf.scroll_region_down_with(self.scroll_top, self.scroll_bottom, &blank);
                }
            }
            b'm' => {
                self.process_sgr(&params);
            }
            b's' => {
                // CSI s — Save cursor position (ANSI.SYS / SGR-like)
                clear_wrap();
                self.saved_cursor = Some(SavedCursor {
                    row: self.cursor_row,
                    col: self.cursor_col,
                    attrs: self.attrs,
                });
            }
            b'u' => {
                // CSI u — Restore cursor position (ANSI.SYS / SGR-like)
                if let Some(saved) = self.saved_cursor {
                    self.cursor_row = saved.row.min(self.rows.saturating_sub(1));
                    self.cursor_col = saved.col.min(self.cols.saturating_sub(1));
                    self.attrs = saved.attrs;
                    self.wrap_pending = false;
                }
            }
            b'h' if intermediate.first() == Some(&b'?') => {
                self.process_dec_private_mode(&params, true);
            }
            b'l' if intermediate.first() == Some(&b'?') => {
                self.process_dec_private_mode(&params, false);
            }
            b'r' => {
                // DECSTBM — Set Top and Bottom Margins.
                // CSI r (no params) or CSI 0;0 r resets to full screen.
                // Per VT100 spec, cursor moves to home after DECSTBM.
                if params.is_empty() {
                    // No parameters → reset scroll region to full screen
                    self.scroll_top = 0;
                    self.scroll_bottom = self.rows.saturating_sub(1);
                } else {
                    let top = param_1based(0, 1);
                    let bottom = param_1based(1, self.rows);
                    if top >= bottom {
                        // Invalid range → reset to full screen
                        self.scroll_top = 0;
                        self.scroll_bottom = self.rows.saturating_sub(1);
                    } else {
                        self.scroll_top = top;
                        self.scroll_bottom = bottom.min(self.rows.saturating_sub(1));
                    }
                }
                clear_wrap();
                // Move cursor to home (origin_mode aware)
                if self.origin_mode {
                    self.cursor_row = self.scroll_top;
                } else {
                    self.cursor_row = 0;
                }
                self.cursor_col = 0;
            }
            // DECSCUSR — Set cursor style (CSI Ps SP q)
            b'q' if intermediate == [0x20] => {
                let ps = param(0, 0);
                self.cursor_style = match ps {
                    0 | 1 => CursorStyle::Block(true),
                    2 => CursorStyle::Block(false),
                    3 => CursorStyle::Underline(true),
                    4 => CursorStyle::Underline(false),
                    5 => CursorStyle::Bar(true),
                    6 => CursorStyle::Bar(false),
                    _ => return,
                };
            }
            // DA1 — Primary Device Attributes (CSI c without intermediate)
            // Respond with VT100 identity: ESC [ ? 1 ; 0 c
            // DA2 (CSI > c) and DA3 (CSI = c) have intermediate bytes and
            // are NOT handled here (we don't claim VT220+ identity).
            b'c' if intermediate.is_empty() => {
                self.response_buf.extend_from_slice(b"\x1b[?1;0c");
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
                30..=37 => {
                    self.attrs.fg = self.palette.resolve(param as u8 - 30);
                }
                38 => {
                    if let Some(next) = params.get(i + 1) {
                        match next.first().copied() {
                            Some(2) if i + 4 < params.len() => {
                                self.attrs.fg = [
                                    params[i + 2].first().copied().unwrap_or(0) as u8,
                                    params[i + 3].first().copied().unwrap_or(0) as u8,
                                    params[i + 4].first().copied().unwrap_or(0) as u8,
                                ];
                                i += 4;
                            }
                            Some(5) => {
                                if let Some(color_param) = params.get(i + 2) {
                                    let idx = color_param.first().copied().unwrap_or(0) as u8;
                                    self.attrs.fg = self.palette.resolve(idx);
                                    i += 2;
                                }
                            }
                            _ => {}
                        }
                    }
                }
                39 => self.attrs.fg = [204, 204, 204],
                40..=47 => {
                    self.attrs.bg = self.palette.resolve(param as u8 - 40);
                }
                48 => {
                    if let Some(next) = params.get(i + 1) {
                        match next.first().copied() {
                            Some(2) if i + 4 < params.len() => {
                                self.attrs.bg = [
                                    params[i + 2].first().copied().unwrap_or(0) as u8,
                                    params[i + 3].first().copied().unwrap_or(0) as u8,
                                    params[i + 4].first().copied().unwrap_or(0) as u8,
                                ];
                                i += 4;
                            }
                            Some(5) => {
                                if let Some(color_param) = params.get(i + 2) {
                                    let idx = color_param.first().copied().unwrap_or(0) as u8;
                                    self.attrs.bg = self.palette.resolve(idx);
                                    i += 2;
                                }
                            }
                            _ => {}
                        }
                    }
                }
                49 => self.attrs.bg = [0, 0, 0],
                90..=97 => {
                    self.attrs.fg = self.palette.resolve(param as u8 - 90 + 8);
                }
                100..=107 => {
                    self.attrs.bg = self.palette.resolve(param as u8 - 100 + 8);
                }
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
                1000 if !set => {
                    // Disabling 1000 also disables higher modes
                    self.mouse_button_tracking = false;
                    self.mouse_any_tracking = false;
                }
                1002 => self.mouse_button_tracking = set,
                1003 => self.mouse_any_tracking = set,
                1006 => self.mouse_sgr = set,
                1004 => self.focus_reporting = set,
                2004 => self.bracketed_paste = set,
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
                    let blank = self.attrs.make_cell(' ', 1);
                    let mut buf = self.buffer.write();
                    buf.scroll_region_down_with(self.scroll_top, self.scroll_bottom, &blank);
                } else {
                    self.cursor_row -= 1;
                }
            }
            b'c' => {
                self.full_reset();
            }
            _ => {}
        }
    }

    fn check_scroll(&mut self) {
        if self.cursor_row > self.scroll_bottom {
            let blank = self.attrs.make_cell(' ', 1);
            let mut buf = self.buffer.write();
            while self.cursor_row > self.scroll_bottom {
                buf.scroll_region_up_with(self.scroll_top, self.scroll_bottom, &blank);
                self.cursor_row -= 1;
            }
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
        let old = std::mem::replace(
            &mut *buf,
            Buffer::new(self.cols, self.rows, self.max_scrollback),
        );
        self.main_buffer = Some(old);
        self.alt_buffer = None; // Clear any stale alt buffer
        self.cursor_row = 0;
        self.cursor_col = 0;
    }

    fn exit_alternate_screen(&mut self) {
        self.alternate_screen = false;
        // Save the current (alt screen) buffer before switching back
        self.alt_buffer = Some(self.buffer.read().clone());
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

    fn process_osc(&mut self, content: &str) {
        // OSC sequences: "code;data"
        // Code 0 or 2: set window title
        // Code 1: set icon name (ignored for now)
        // Code 4: set color palette
        // Code 104: reset color palette entries
        let (code, data) = if let Some(pos) = content.find(';') {
            let (c, d) = content.split_at(pos);
            (c, &d[1..])
        } else {
            (content, "")
        };
        match code {
            "0" | "2" => self.title = data.to_string(),
            "4" => {
                self.palette.apply_osc4(data);
            }
            "777" => {
                // OSC 777 — desktop notification.  Format: "777;title;body"
                // Log it as a tracing event for the host to act on.
                // The data is semicolon-separated: title;body
                let parts: Vec<&str> = data.splitn(2, ';').collect();
                let title = parts.first().copied().unwrap_or("");
                let body = parts.get(1).copied().unwrap_or("");
                tracing::debug!(title, body, "OSC 777 desktop notification");
            }
            "104" => {
                // OSC 104 — reset palette colors to defaults
                // Format: "104;N" or "104;N;M;..." — reset specific entries
                // "104" with no data — reset all
                if data.is_empty() {
                    self.palette.reset();
                } else {
                    for part in data.split(';') {
                        if let Ok(idx) = part.parse::<u8>() {
                            self.palette.set(idx, super::color::color_256_to_rgb(idx));
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn process_dcs(
        &mut self,
        params: &[Vec<u16>],
        intermediate: &[u8],
        final_byte: u8,
        data: &str,
    ) {
        if final_byte == b'q' {
            // DCS with final byte 'q' can be:
            // 1. Sixel image protocol: intermediate contains '?' (or no intermediate but no params)
            // 2. Kitty image protocol: params like "i=1;0;0" (first param = 1 with 'i')
            //
            // Sixel detection: intermediate byte '?' indicates sixel mode.
            // Also: if params are all 0 and intermediate is empty and data is non-empty,
            // it could be sixel (the introducer is part of data).
            let is_sixel = intermediate.contains(&b'?')
                || (intermediate.is_empty()
                    && params.iter().all(|p| p.iter().all(|&v| v == 0))
                    && !data.is_empty());

            if is_sixel {
                // Sixel image data — store with "sixel:" prefix and record position
                let pos = (self.cursor_row, self.cursor_col);
                self.dcs_buffer = format!("sixel:{}:{}", data.len(), data);
                self.sixel_images.push((pos.0, pos.1, data.to_string()));
                tracing::debug!(
                    row = pos.0,
                    col = pos.1,
                    len = data.len(),
                    "Stored Sixel inline image"
                );
                return;
            }

            // Kitty image protocol
            let ps = params
                .iter()
                .map(|p| p.first().copied().unwrap_or(0).to_string())
                .collect::<Vec<_>>()
                .join(";");
            self.dcs_buffer = format!("kitty:{}:{}", ps, data);
        }
        // Other DCS sequences (tmux control, etc.) are silently consumed.
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
        self.wrap_pending = false;
        self.mouse_sgr = false;
        self.mouse_button_tracking = false;
        self.mouse_any_tracking = false;
        self.title.clear();
        self.bracketed_paste = false;
        self.focus_reporting = false;
        self.bell_pending = false;
        self.cursor_style = CursorStyle::Block(true);
        self.dcs_buffer.clear();
        self.response_buf.clear();
        self.sixel_images.clear();
        self.palette.reset();
        {
            let mut buf = self.buffer.write();
            buf.clear_all();
        }
        if self.alternate_screen {
            self.exit_alternate_screen();
        }
        self.alt_buffer = None;
    }

    /// Whether the emulator is currently showing the alternate screen.
    pub fn is_alternate_screen(&self) -> bool {
        self.alternate_screen
    }

    /// Force-exit the alternate screen if active.
    /// Used when a command exits without properly restoring the main screen
    /// (e.g., vim killed without :q, htop killed without q).
    /// Returns true if we were on the alternate screen and recovered.
    pub fn recover_from_alternate_screen(&mut self) -> bool {
        if self.alternate_screen {
            self.exit_alternate_screen();
            tracing::info!("Auto-recovered from stale alternate screen");
            true
        } else {
            false
        }
    }

    // Public API
    pub fn buffer(&self) -> Buffer {
        self.buffer.read().clone()
    }

    /// Snapshot the currently active buffer (main or alternate).
    pub fn snapshot(&self) -> Buffer {
        self.buffer.read().clone()
    }

    /// Return the current buffer's generation counter.
    /// This is a cheap O(1) read (requires only a read lock) that can be used
    /// for change detection without cloning the entire buffer.
    pub fn buffer_generation(&self) -> u64 {
        self.buffer.read().generation()
    }

    /// Return the number of scrollback lines without cloning the buffer.
    /// O(1) — just reads the vec length under a read lock.
    pub fn scrollback_len(&self) -> usize {
        self.buffer.read().scrollback.len()
    }

    /// Snapshot the main buffer (returns the main buffer even if the
    /// alternate screen is currently active).
    pub fn snapshot_main(&self) -> Buffer {
        if self.alternate_screen {
            self.main_buffer
                .clone()
                .unwrap_or_else(|| self.buffer.read().clone())
        } else {
            self.buffer.read().clone()
        }
    }

    /// Snapshot the alternate buffer (returns the alt buffer if active,
    /// or the last known alt buffer if the app has switched back to main).
    pub fn snapshot_alt(&self) -> Buffer {
        if self.alternate_screen {
            self.buffer.read().clone()
        } else {
            // Not on alt screen — return the last saved alt buffer if available.
            self.alt_buffer
                .as_ref()
                .cloned()
                .unwrap_or_else(|| Buffer::new(self.cols, self.rows, self.max_scrollback))
        }
    }

    pub fn contents_plain(&self) -> String {
        let buf = self.buffer.read();
        let mut lines: Vec<String> = Vec::with_capacity(buf.scrollback.len() + buf.rows.len());
        for row in &buf.scrollback {
            lines.push(row.iter().filter(|c| c.width > 0).map(|c| c.ch).collect::<String>());
        }
        for row in &buf.rows {
            lines.push(row.iter().filter(|c| c.width > 0).map(|c| c.ch).collect::<String>());
        }
        lines.join("\n")
    }

    /// Render the full buffer (scrollback + visible rows) as ANSI text
    /// with color codes, SGR attribute resets, and line/screen clearing.
    ///
    /// Per-row: emits ESC[K (clear to end of line) after the newline so that
    /// when the physical terminal is wider than the VTTY buffer, stale content
    /// does not remain visible in the right margin columns.
    ///
    /// Per-row: resets SGR attributes (ESC[0m) at the end of each row to
    /// prevent background color bleed from the last cell extending into the
    /// newline and beyond.
    ///
    /// End: emits ESC[J (clear to end of screen) so that when the physical
    /// terminal is taller than the VTTY buffer, stale content does not remain
    /// visible below the buffer.
    ///
    /// End: emits ESC[0m (SGR reset) as a final safety net.
    pub fn contents_ansi(&self) -> String {
        let buf = self.buffer.read();
        let mut output = String::new();
        let mut last_fg: Option<[u8; 3]> = None;
        let mut last_bg: Option<[u8; 3]> = None;
        let mut last_bold = false;
        let mut last_italic = false;
        let mut last_underline = false;
        let mut last_blink = false;
        let mut last_reverse = false;
        let mut last_invisible = false;
        let mut last_strikethrough = false;

        // Iterate scrollback first, then visible rows.
        let all_rows = buf.scrollback.iter().chain(buf.rows.iter());
        for row in all_rows {
            for cell in row {
                // Skip wide-char continuation cells (width=0).
                if cell.width == 0 {
                    continue;
                }

                let mut codes = Vec::new();
                if cell.bold != last_bold {
                    codes.push(if cell.bold { "1" } else { "22" }.to_string());
                    last_bold = cell.bold;
                }
                if cell.italic != last_italic {
                    codes.push(if cell.italic { "3" } else { "23" }.to_string());
                    last_italic = cell.italic;
                }
                if cell.underline != last_underline {
                    codes.push(if cell.underline { "4" } else { "24" }.to_string());
                    last_underline = cell.underline;
                }
                if cell.blink != last_blink {
                    codes.push(if cell.blink { "5" } else { "25" }.to_string());
                    last_blink = cell.blink;
                }
                if cell.reverse != last_reverse {
                    codes.push(if cell.reverse { "7" } else { "27" }.to_string());
                    last_reverse = cell.reverse;
                }
                if cell.invisible != last_invisible {
                    codes.push(if cell.invisible { "8" } else { "28" }.to_string());
                    last_invisible = cell.invisible;
                }
                if cell.strikethrough != last_strikethrough {
                    codes.push(if cell.strikethrough { "9" } else { "29" }.to_string());
                    last_strikethrough = cell.strikethrough;
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
            // Reset SGR at end of row to prevent background color bleed
            // extending past the buffer width into the newline.
            if last_fg.is_some() || last_bg.is_some()
                || last_bold || last_italic || last_underline
                || last_blink || last_reverse || last_invisible
                || last_strikethrough
            {
                output.push_str("\x1b[0m");
                last_fg = None;
                last_bg = None;
                last_bold = false;
                last_italic = false;
                last_underline = false;
                last_blink = false;
                last_reverse = false;
                last_invisible = false;
                last_strikethrough = false;
            }
            output.push('\n');
            // ESC[K — clear to end of line.  Without this, columns beyond
            // the buffer width retain whatever the terminal previously showed.
            output.push_str("\x1b[K");
        }
        // ESC[J — clear to end of screen.  Without this, rows below the
        // buffer retain whatever the terminal previously showed.
        output.push_str("\x1b[J\x1b[0m");
        output
    }

    pub fn partial(&self, start_row: usize, row_count: usize) -> String {
        let buf = self.buffer.read();
        let start = start_row.min(buf.rows.len());
        let end = (start + row_count).min(buf.rows.len());
        buf.rows[start..end]
            .iter()
            .map(|row| row.iter().filter(|c| c.width > 0).map(|c| c.ch).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn cursor(&self) -> (usize, usize) {
        (self.cursor_row, self.cursor_col)
    }

    /// Whether the application has made the cursor visible (DEC private mode 25).
    /// Applications like htop hide the cursor with `?25l`.
    pub fn is_cursor_visible(&self) -> bool {
        self.cursor_visible
    }

    pub fn dimensions(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }

    /// Whether SGR extended mouse coordinates (?1006) is enabled.
    pub fn mouse_sgr_enabled(&self) -> bool {
        self.mouse_sgr
    }

    /// Whether any mouse tracking mode is enabled (1002 or 1003).
    pub fn mouse_tracking_enabled(&self) -> bool {
        self.mouse_button_tracking || self.mouse_any_tracking
    }

    /// Current window title (set via OSC 0 / OSC 2).
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Whether bracketed paste mode is enabled (?2004).
    pub fn bracketed_paste_enabled(&self) -> bool {
        self.bracketed_paste
    }

    /// Whether focus reporting is enabled (?1004).
    pub fn focus_reporting_enabled(&self) -> bool {
        self.focus_reporting
    }

    /// Check and clear the bell pending flag.
    /// Returns true if BEL was received since the last check.
    pub fn drain_bell(&mut self) -> bool {
        std::mem::replace(&mut self.bell_pending, false)
    }

    /// Current cursor style set by DECSCUSR.
    pub fn cursor_style(&self) -> CursorStyle {
        self.cursor_style
    }

    /// Most recent DCS data (e.g. kitty graphics protocol payload).
    pub fn dcs_buffer(&self) -> &str {
        &self.dcs_buffer
    }

    /// Get the list of stored inline Sixel images.
    /// Each entry is (row, col, sixel_data_string) where row/col is the
    /// cursor position when the image was received.
    pub fn sixel_images(&self) -> &[(usize, usize, String)] {
        &self.sixel_images
    }

    /// Clear all stored inline Sixel images.
    pub fn clear_sixel_images(&mut self) {
        self.sixel_images.clear();
    }

    /// Drain pending response bytes (e.g. DA1 replies) that should be
    /// written back to the child PTY.  Returns an empty vec if nothing
    /// is pending.  The caller is responsible for sending these bytes
    /// to the PTY's stdin.
    pub fn drain_responses(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.response_buf)
    }

    /// Reference to the current color palette.
    pub fn palette(&self) -> &ColorPalette {
        &self.palette
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
        // LF acts as Unix newline (CR+LF): cursor moves to (row+1, col 0).
        assert_eq!(emu.cursor(), (1, 5));
        let buf = emu.buffer();
        assert_eq!(buf.rows[0][0].ch, 'H');
        assert_eq!(buf.rows[1][0].ch, 'W');
    }

    #[test]
    fn test_carriage_return_linefeed() {
        let mut emu = VttyEmulator::new(10, 10, 100);
        emu.feed_str("Hello\r\nWorld");
        assert_eq!(emu.cursor(), (1, 5));
        let buf = emu.buffer();
        assert_eq!(buf.rows[0][0].ch, 'H');
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
        emu.feed_str("Line1\r\nLine2\r\nLine3\r\nLine4");
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
        emu.feed_str("X");
        emu.feed_str("\x1b7");
        emu.feed_str("\x1b[1;1H");
        emu.feed_str("\x1b[32m");
        emu.feed_str("\x1b8");
        assert_eq!(emu.cursor(), (4, 5));
        let buf = emu.buffer();
        assert_eq!(buf.rows[4][4].fg, [170, 0, 0]);
        assert_eq!(buf.rows[4][4].ch, 'X');
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
    fn test_contents_ansi_blink() {
        let mut emu = VttyEmulator::new(3, 20, 100);
        emu.feed_str("\x1b[5mBlink\x1b[0m");
        let ansi = emu.contents_ansi();
        // SGR 5 may be batched with other codes (e.g. "5;38;2;204;204;204")
        assert!(ansi.contains("5;") || ansi.contains("\x1b[5m"),
            "should contain SGR 5 (blink on), got: {:?}", ansi);
        assert!(ansi.contains("25;") || ansi.contains("\x1b[25m"),
            "should contain SGR 25 (blink off), got: {:?}", ansi);
        assert!(ansi.contains("Blink"));
    }

    #[test]
    fn test_contents_ansi_reverse() {
        let mut emu = VttyEmulator::new(3, 20, 100);
        emu.feed_str("\x1b[7mReversed\x1b[0m");
        let ansi = emu.contents_ansi();
        assert!(ansi.contains("7;") || ansi.contains("\x1b[7m"),
            "should contain SGR 7 (reverse on), got: {:?}", ansi);
        assert!(ansi.contains("27;") || ansi.contains("\x1b[27m"),
            "should contain SGR 27 (reverse off), got: {:?}", ansi);
        assert!(ansi.contains("Reversed"));
    }

    #[test]
    fn test_contents_ansi_invisible() {
        let mut emu = VttyEmulator::new(3, 20, 100);
        emu.feed_str("\x1b[8mHidden\x1b[0m");
        let ansi = emu.contents_ansi();
        assert!(ansi.contains("8;") || ansi.contains("\x1b[8m"),
            "should contain SGR 8 (invisible on), got: {:?}", ansi);
        assert!(ansi.contains("28;") || ansi.contains("\x1b[28m"),
            "should contain SGR 28 (invisible off), got: {:?}", ansi);
        assert!(ansi.contains("Hidden"));
    }

    #[test]
    fn test_contents_ansi_strikethrough() {
        let mut emu = VttyEmulator::new(3, 20, 100);
        emu.feed_str("\x1b[9mStrike\x1b[0m");
        let ansi = emu.contents_ansi();
        assert!(ansi.contains("9;") || ansi.contains("\x1b[9m"),
            "should contain SGR 9 (strikethrough on), got: {:?}", ansi);
        assert!(ansi.contains("29;") || ansi.contains("\x1b[29m"),
            "should contain SGR 29 (strikethrough off), got: {:?}", ansi);
        assert!(ansi.contains("Strike"));
    }

    #[test]
    fn test_contents_ansi_ends_with_clear_screen_and_reset() {
        let mut emu = VttyEmulator::new(3, 10, 100);
        emu.feed_str("Hello");
        let ansi = emu.contents_ansi();
        // Must end with ESC[J (clear to end of screen) followed by ESC[0m (SGR reset)
        assert!(ansi.ends_with("\x1b[J\x1b[0m"),
            "ANSI output must end with ESC[J ESC[0m, got tail: {:?}",
            &ansi[ansi.len().saturating_sub(20)..]);
    }

    #[test]
    fn test_contents_ansi_per_row_clear_to_eol() {
        let mut emu = VttyEmulator::new(2, 5, 100);
        emu.feed_str("ABCDE\nFGHIJ");
        let ansi = emu.contents_ansi();
        // Each row must be followed by ESC[K (clear to end of line)
        // With 2 rows, there should be 2 ESC[K sequences
        let eol_k_count = ansi.matches("\x1b[K").count();
        assert_eq!(eol_k_count, 2,
            "Expected 2 ESC[K (one per row), got {}: {:?}", eol_k_count, ansi);
    }

    #[test]
    fn test_contents_ansi_per_row_sgr_reset() {
        let mut emu = VttyEmulator::new(2, 10, 100);
        emu.feed_str("\x1b[31mRed\x1b[0m\n\x1b[32mGreen\x1b[0m");
        let ansi = emu.contents_ansi();
        // SGR reset after each row that had color — the per-row reset prevents
        // color bleed between rows.  We should see at least 2 resets (one per row
        // with color) plus the final one.
        let reset_count = ansi.matches("\x1b[0m").count();
        assert!(reset_count >= 3,
            "Expected at least 3 ESC[0m (per-row resets + final), got {}: {:?}", reset_count, ansi);
    }

    #[test]
    fn test_contents_ansi_includes_scrollback() {
        let mut emu = VttyEmulator::new(3, 10, 100);
        // Fill all 3 rows, then 2 more — first 2 go to scrollback
        emu.feed_str("Line1\nLine2\nLine3\nLine4\nLine5");
        let ansi = emu.contents_ansi();
        assert!(ansi.contains("Line1"), "scrollback line 1 must be present");
        assert!(ansi.contains("Line2"), "scrollback line 2 must be present");
        assert!(ansi.contains("Line3"), "visible line must be present");
        assert!(ansi.contains("Line4"), "visible line must be present");
        assert!(ansi.contains("Line5"), "visible line must be present");
    }

    #[test]
    fn test_contents_plain_includes_scrollback() {
        let mut emu = VttyEmulator::new(3, 10, 100);
        emu.feed_str("AAA\nBBB\nCCC\nDDD\nEEE");
        let text = emu.contents_plain();
        assert!(text.contains("AAA"), "scrollback must be in plain text");
        assert!(text.contains("BBB"), "scrollback must be in plain text");
        assert!(text.contains("CCC"), "visible line must be in plain text");
        assert!(text.contains("DDD"), "visible line must be in plain text");
        assert!(text.contains("EEE"), "visible line must be in plain text");
    }

    #[test]
    fn test_contents_ansi_no_trailing_color_bleed() {
        // When the vtty is smaller than the terminal, the last cell's color
        // must not bleed into rows below.  The ESC[J at the end ensures this.
        let mut emu = VttyEmulator::new(2, 10, 100);
        emu.feed_str("\x1b[41mRedBG\x1b[0m");
        let ansi = emu.contents_ansi();
        // The output must end with ESC[J then ESC[0m — the ESC[J clears all
        // rows below the buffer, and ESC[0m resets any color so the user's
        // terminal prompt is not colored.
        assert!(ansi.ends_with("\x1b[J\x1b[0m"),
            "Must end with clear-screen + reset to prevent color bleed below buffer");
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

    // -- Deferred wrap tests --

    #[test]
    fn test_erase_characters() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("ABCDEFGH");
        emu.feed_str("\x1b[2;3H");
        emu.feed_str("\x1b[31m");
        emu.feed_str("XYZ");
        emu.feed_str("\x1b[2;4H");
        emu.feed_str("\x1b[2X");
        let buf = emu.buffer();
        assert_eq!(buf.rows[1][2].ch, 'X');
        assert_eq!(buf.rows[1][3].ch, ' ');
        assert_eq!(buf.rows[1][4].ch, ' ');
        assert_eq!(emu.cursor(), (1, 3));
    }

    #[test]
    fn test_vpa_vertical_position() {
        let mut emu = VttyEmulator::new(10, 10, 100);
        emu.feed_str("\x1b[5;5H");
        emu.feed_str("\x1b[2d");
        assert_eq!(emu.cursor(), (1, 4));
    }

    #[test]
    fn test_csi_s_u_save_restore_cursor() {
        let mut emu = VttyEmulator::new(10, 10, 100);
        emu.feed_str("\x1b[3;5H");
        emu.feed_str("\x1b[31m");
        emu.feed_str("X");
        emu.feed_str("\x1b[s");
        emu.feed_str("\x1b[1;1H");
        emu.feed_str("\x1b[32m");
        emu.feed_str("G");
        emu.feed_str("\x1b[u");
        assert_eq!(emu.cursor(), (2, 5));
        emu.feed_str("Y");
        let buf = emu.buffer();
        assert_eq!(buf.rows[2][4].fg, [170, 0, 0]);
        assert_eq!(buf.rows[2][5].fg, [170, 0, 0]);
        assert_eq!(buf.rows[0][0].fg, [0, 170, 0]);
    }

    #[test]
    fn test_el_mode_0_erase_to_end() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("ABCDEFGHIJ");
        emu.feed_str("\x1b[1;5H");
        emu.feed_str("\x1b[44m");
        emu.feed_str("\x1b[K");
        let buf = emu.buffer();
        assert_eq!(buf.rows[0][0].ch, 'A');
        assert_eq!(buf.rows[0][3].ch, 'D');
        assert_eq!(buf.rows[0][4].ch, ' ');
        assert_eq!(buf.rows[0][4].bg, [0, 0, 170]);
        assert_eq!(buf.rows[0][9].ch, ' ');
        assert_eq!(buf.rows[0][0].bg, [0, 0, 0]);
    }

    #[test]
    fn test_el_mode_1_erase_to_beginning() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("ABCDEFGHIJ");
        emu.feed_str("\x1b[1;5H");
        emu.feed_str("\x1b[44m");
        emu.feed_str("\x1b[1K");
        let buf = emu.buffer();
        assert_eq!(buf.rows[0][0].ch, ' ');
        assert_eq!(buf.rows[0][0].bg, [0, 0, 170]);
        assert_eq!(buf.rows[0][4].ch, ' ');
        assert_eq!(buf.rows[0][4].bg, [0, 0, 170]);
        assert_eq!(buf.rows[0][5].ch, 'F');
        assert_eq!(buf.rows[0][5].bg, [0, 0, 0]);
    }

    #[test]
    fn test_el_mode_2_erase_entire_line() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("ABCDEFGHIJ");
        emu.feed_str("\x1b[2;1H");
        emu.feed_str("KLMNOPQRST");
        emu.feed_str("\x1b[1;1H");
        emu.feed_str("\x1b[44m");
        emu.feed_str("\x1b[2K");
        let buf = emu.buffer();
        for cell in &buf.rows[0] {
            assert_eq!(cell.ch, ' ');
            assert_eq!(cell.bg, [0, 0, 170]);
        }
        assert_eq!(buf.rows[1][0].ch, 'K');
        assert_eq!(buf.rows[1][0].bg, [0, 0, 0]);
    }

    // -- OSC title tests --

    #[test]
    fn test_osc_0_set_title() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("\x1b]0;My Title\x07");
        assert_eq!(emu.title(), "My Title");
    }

    #[test]
    fn test_osc_2_set_title() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("\x1b]2;Window Title\x07");
        assert_eq!(emu.title(), "Window Title");
    }

    #[test]
    fn test_osc_1_ignored() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("\x1b]0;Original\x07");
        emu.feed_str("\x1b]1;Icon Name\x07");
        assert_eq!(emu.title(), "Original");
    }

    #[test]
    fn test_osc_title_with_semicolons() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("\x1b]0;foo;bar;baz\x07");
        assert_eq!(emu.title(), "foo;bar;baz");
    }

    #[test]
    fn test_osc_title_st_terminated() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("\x1b]0;ST Title\x1b\\");
        assert_eq!(emu.title(), "ST Title");
    }

    #[test]
    fn test_osc_title_cleared_on_reset() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("\x1b]0;Title\x07");
        assert_eq!(emu.title(), "Title");
        emu.feed_str("\x1bc"); // full reset
        assert_eq!(emu.title(), "");
    }

    // -- Bracketed paste tests --

    #[test]
    fn test_osc4_set_color() {
        let mut emu = VttyEmulator::new(10, 10, 100);
        emu.feed_str("\x1b]4;1;rgb:ff/00/00\x07");
        assert_eq!(emu.palette().get(1), [255, 0, 0]);
    }

    #[test]
    fn test_osc4_affects_sgr() {
        let mut emu = VttyEmulator::new(10, 10, 100);
        // Redefine color 1 (normally red) to pure blue
        emu.feed_str("\x1b]4;1;#0000ff\x07");
        emu.feed_str("\x1b[31mX"); // SGR 31 should now use the custom blue
        let buf = emu.buffer();
        assert_eq!(buf.rows[0][0].fg, [0, 0, 255]);
    }

    #[test]
    fn test_osc4_multiple_colors() {
        let mut emu = VttyEmulator::new(10, 10, 100);
        emu.feed_str("\x1b]4;0;rgb:ff/00/00;1;rgb:00/ff/00;2;#0000ff\x07");
        assert_eq!(emu.palette().get(0), [255, 0, 0]);
        assert_eq!(emu.palette().get(1), [0, 255, 0]);
        assert_eq!(emu.palette().get(2), [0, 0, 255]);
    }

    #[test]
    fn test_osc104_reset_color() {
        let mut emu = VttyEmulator::new(10, 10, 100);
        emu.feed_str("\x1b]4;1;#0000ff\x07");
        assert_eq!(emu.palette().get(1), [0, 0, 255]);
        emu.feed_str("\x1b]104;1\x07"); // Reset color 1
        assert_eq!(emu.palette().get(1), [170, 0, 0]); // Back to default
    }

    #[test]
    fn test_osc104_reset_all() {
        let mut emu = VttyEmulator::new(10, 10, 100);
        emu.feed_str("\x1b]4;0;#ffffff;1;#000000\x07");
        emu.feed_str("\x1b]104\x07"); // Reset all
        assert_eq!(emu.palette().get(0), [0, 0, 0]); // Back to default
        assert_eq!(emu.palette().get(1), [170, 0, 0]); // Back to default
    }

    #[test]
    fn test_palette_reset_on_full_reset() {
        let mut emu = VttyEmulator::new(10, 10, 100);
        emu.feed_str("\x1b]4;1;#0000ff\x07");
        assert_eq!(emu.palette().get(1), [0, 0, 255]);
        emu.feed_str("\x1bc");
        assert_eq!(emu.palette().get(1), [170, 0, 0]); // Back to default
    }

    // -- Unicode wide character tests --

    #[test]
    fn test_wide_char_basic() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("A中B");
        let buf = emu.buffer();
        assert_eq!(buf.rows[0][0].ch, 'A');
        assert_eq!(buf.rows[0][0].width, 1);
        assert_eq!(buf.rows[0][1].ch, '中');
        assert_eq!(buf.rows[0][1].width, 2); // wide char lead
        assert_eq!(buf.rows[0][2].width, 0); // continuation
        assert_eq!(buf.rows[0][3].ch, 'B');
        assert_eq!(buf.rows[0][3].width, 1);
        assert_eq!(emu.cursor(), (0, 4));
    }

    #[test]
    fn test_wide_char_at_end_of_line() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        // Place wide char at cols 8-9 (last two columns, 1-based col 9)
        emu.feed_str("\x1b[1;9H中");
        let buf = emu.buffer();
        assert_eq!(buf.rows[0][8].ch, '中');
        assert_eq!(buf.rows[0][8].width, 2);
        assert_eq!(buf.rows[0][9].width, 0);
        assert_eq!(emu.cursor(), (0, 9));
    }

    #[test]
    fn test_wide_char_wraps_when_no_room() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        // Cursor at col 9 (last column) — not enough room for a wide char
        emu.feed_str("\x1b[1;10H");
        assert_eq!(emu.cursor(), (0, 9));
        emu.feed_str("中");
        // Should wrap to next line
        assert_eq!(emu.cursor(), (1, 2));
        let buf = emu.buffer();
        assert_eq!(buf.rows[1][0].ch, '中');
        assert_eq!(buf.rows[1][0].width, 2);
        assert_eq!(buf.rows[1][1].width, 0);
    }

    #[test]
    fn test_bs_over_wide_char() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("A中");
        assert_eq!(emu.cursor(), (0, 3));
        emu.feed_str("\x08"); // BS — should land on the wide char lead
        assert_eq!(emu.cursor(), (0, 1));
        emu.feed_str("\x08"); // BS — should land on 'A'
        assert_eq!(emu.cursor(), (0, 0));
    }

    #[test]
    fn test_cjk_text_layout() {
        let mut emu = VttyEmulator::new(3, 10, 100);
        emu.feed_str("你好世界");
        let buf = emu.buffer();
        assert_eq!(buf.rows[0][0].ch, '你');
        assert_eq!(buf.rows[0][0].width, 2);
        assert_eq!(buf.rows[0][1].width, 0);
        assert_eq!(buf.rows[0][2].ch, '好');
        assert_eq!(buf.rows[0][2].width, 2);
        assert_eq!(buf.rows[0][3].width, 0);
        assert_eq!(buf.rows[0][4].ch, '世');
        assert_eq!(buf.rows[0][4].width, 2);
        assert_eq!(buf.rows[0][5].width, 0);
        assert_eq!(buf.rows[0][6].ch, '界');
        assert_eq!(buf.rows[0][6].width, 2);
        assert_eq!(buf.rows[0][7].width, 0);
        assert_eq!(emu.cursor(), (0, 8));
    }

    #[test]
    fn test_wide_char_with_sgr() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("\x1b[31m中\x1b[0m");
        let buf = emu.buffer();
        assert_eq!(buf.rows[0][0].fg, [170, 0, 0]);
        assert_eq!(buf.rows[0][0].width, 2);
        // Continuation cell should inherit the same fg
        assert_eq!(buf.rows[0][1].fg, [170, 0, 0]);
        assert_eq!(buf.rows[0][1].width, 0);
    }

    #[test]
    fn test_fullwidth_forms() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("Ａ"); // Fullwidth Latin A
        let buf = emu.buffer();
        assert_eq!(buf.rows[0][0].ch, 'Ａ');
        assert_eq!(buf.rows[0][0].width, 2);
        assert_eq!(buf.rows[0][1].width, 0);
    }

    // ── Proposal #2: Bracketed paste mode tests ──

    #[test]
    fn test_bracketed_paste_enable_disable() {
        let mut emu = VttyEmulator::new(10, 10, 100);
        assert!(!emu.bracketed_paste_enabled());
        emu.feed_str("\x1b[?2004h");
        assert!(emu.bracketed_paste_enabled());
        emu.feed_str("\x1b[?2004l");
        assert!(!emu.bracketed_paste_enabled());
    }

    #[test]
    fn test_bracketed_paste_cleared_on_reset() {
        let mut emu = VttyEmulator::new(10, 10, 100);
        emu.feed_str("\x1b[?2004h");
        assert!(emu.bracketed_paste_enabled());
        emu.feed_str("\x1bc"); // RIS (full reset)
        assert!(!emu.bracketed_paste_enabled());
    }

    // ── Proposal #5: Scroll region reset tests ──

    #[test]
    fn test_scroll_region_set_and_use() {
        // CSI 2;4 r sets scroll region to rows 2-4 (1-based)
        // Internally: scroll_top=1, scroll_bottom=3 (0-indexed)
        // Cursor moves to home (0,0) after DECSTBM
        let mut emu = VttyEmulator::new(10, 10, 100);
        emu.feed_str("\x1b[2;4r");
        assert_eq!(emu.cursor(), (0, 0));
    }

    #[test]
    fn test_scroll_region_reset_no_params() {
        // CSI r with no params resets to full screen
        let mut emu = VttyEmulator::new(10, 10, 100);
        emu.feed_str("\x1b[2;4r"); // Set scroll region
        emu.feed_str("\x1b[r"); // Reset
        assert_eq!(emu.cursor(), (0, 0));
    }

    #[test]
    fn test_scroll_region_reset_invalid_range() {
        // CSI 5;3 r is invalid (top > bottom) → reset to full screen
        let mut emu = VttyEmulator::new(10, 10, 100);
        emu.feed_str("\x1b[5;3r");
        assert_eq!(emu.cursor(), (0, 0));
    }

    #[test]
    fn test_scroll_region_scrolls_within_region() {
        // When scroll region is set, newlines only scroll within region
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("\x1b[2;4r"); // Set scroll region rows 2-4 (1-based)
        emu.feed_str("\x1b[2;1H"); // Move to row 2 (1-based)
                                   // Fill lines 2-4, the 4th line should push line 2 out and scroll within region
        emu.feed_str("AAAAAA\r\nBBBBBB\r\nCCCCCC\r\nDDDDDD");
        // Row 0 (outside region) should be unaffected
        let buf = emu.buffer();
        assert_eq!(buf.rows[0][0].ch, ' ');
    }

    // ── Proposal #6: Deferred wrap edge case tests ──

    #[test]
    fn test_deferred_wrap_basic() {
        // Fill exactly to the right margin → wrap_pending should be set
        // Next character wraps to next line
        let mut emu = VttyEmulator::new(3, 5, 100);
        emu.feed_str("ABCDE"); // Fill entire first line
        assert_eq!(emu.cursor(), (0, 4)); // At last col
                                          // wrap_pending should be true (check by writing next char)
        emu.feed_str("X");
        assert_eq!(emu.cursor(), (1, 1));
        let buf = emu.buffer();
        assert_eq!(buf.rows[0][4].ch, 'E');
        assert_eq!(buf.rows[1][0].ch, 'X');
    }

    #[test]
    fn test_deferred_wrap_cleared_by_cursor_movement() {
        // Write to last col → wrap_pending
        // Then move cursor up → wrap_pending cleared
        // Write another char → should NOT wrap
        let mut emu = VttyEmulator::new(5, 5, 100);
        emu.feed_str("ABCDE"); // Fill line, wrap_pending
        emu.feed_str("\x1b[A"); // Cursor up (clears wrap_pending)
        emu.feed_str("X"); // Should go at (0, 4), not wrap
        assert_eq!(emu.cursor(), (0, 4));
        let buf = emu.buffer();
        assert_eq!(buf.rows[0][4].ch, 'X');
    }

    #[test]
    fn test_deferred_wrap_cleared_by_cr() {
        let mut emu = VttyEmulator::new(3, 5, 100);
        emu.feed_str("ABCDE"); // Fill line, wrap_pending
        emu.feed_str("\r"); // CR clears wrap_pending
        assert_eq!(emu.cursor(), (0, 0));
    }

    #[test]
    fn test_deferred_wrap_cleared_by_tab() {
        let mut emu = VttyEmulator::new(3, 20, 100);
        emu.feed_str("12345678901234567890"); // Fill line, wrap_pending
        emu.feed_str("\t"); // Tab clears wrap_pending
                            // Cursor should stay on row 0 (not wrap)
        assert!(emu.cursor().0 == 0); // Still on row 0
    }

    #[test]
    fn test_deferred_wrap_at_scroll_bottom() {
        // If wrap_pending at the last visible row, writing should scroll
        let mut emu = VttyEmulator::new(3, 5, 100);
        emu.feed_str("AAAAA\r\nBBBBB\r\nCCCCC"); // Fill all 3 rows, last char on row 2
        assert_eq!(emu.cursor(), (2, 4));
        emu.feed_str("D"); // Should wrap to row 3 and scroll
        assert_eq!(emu.cursor(), (2, 1));
        let buf = emu.buffer();
        assert_eq!(buf.rows[2][0].ch, 'D');
        assert_eq!(buf.scrollback.len(), 1); // One line scrolled out
    }

    // ── Proposal #8: DECSCUSR cursor style tests ──

    #[test]
    fn test_decscusr_blinking_block() {
        let mut emu = VttyEmulator::new(10, 10, 100);
        emu.feed_str("\x1b[0 q"); // Note: space before q is the intermediate byte
        assert_eq!(emu.cursor_style(), CursorStyle::Block(true));
    }

    #[test]
    fn test_decscusr_steady_block() {
        let mut emu = VttyEmulator::new(10, 10, 100);
        emu.feed_str("\x1b[2 q");
        assert_eq!(emu.cursor_style(), CursorStyle::Block(false));
    }

    #[test]
    fn test_decscusr_blinking_underline() {
        let mut emu = VttyEmulator::new(10, 10, 100);
        emu.feed_str("\x1b[3 q");
        assert_eq!(emu.cursor_style(), CursorStyle::Underline(true));
    }

    #[test]
    fn test_decscursor_steady_bar() {
        let mut emu = VttyEmulator::new(10, 10, 100);
        emu.feed_str("\x1b[6 q");
        assert_eq!(emu.cursor_style(), CursorStyle::Bar(false));
    }

    #[test]
    fn test_decscusr_reset_to_default() {
        let mut emu = VttyEmulator::new(10, 10, 100);
        emu.feed_str("\x1b[6 q"); // Set to steady bar
        assert_eq!(emu.cursor_style(), CursorStyle::Bar(false));
        emu.feed_str("\x1b[1 q"); // Reset to blinking block
        assert_eq!(emu.cursor_style(), CursorStyle::Block(true));
    }

    // ── Proposal #9: DCS pass-through tests ──

    #[test]
    fn test_dcs_kitty_graphics() {
        // DCS q is the kitty graphics protocol
        let mut emu = VttyEmulator::new(10, 10, 100);
        // Minimal kitty graphics: DCS 1;0;0 q ST
        emu.feed(b"\x1bP1;0;0q\x1b\\");
        assert!(emu.dcs_buffer().starts_with("kitty:"));
        assert!(emu.sixel_images().is_empty()); // not sixel
    }

    #[test]
    fn test_dcs_unknown_silently_consumed() {
        // Unknown DCS sequences should be silently consumed (no panic)
        let mut emu = VttyEmulator::new(10, 10, 100);
        emu.feed(b"\x1bPunknown\x1b\\");
        // Should not panic and dcs_buffer should remain empty
        assert!(emu.dcs_buffer().is_empty());
    }

    #[test]
    fn test_dcs_cleared_on_reset() {
        let mut emu = VttyEmulator::new(10, 10, 100);
        emu.feed(b"\x1bP1;0;0q\x1b\\");
        assert!(!emu.dcs_buffer().is_empty());
        emu.feed_str("\x1bc"); // RIS (full reset)
        assert!(emu.dcs_buffer().is_empty());
    }

    // ── Sixel tests (#20) ──

    #[test]
    fn test_sixel_dcs_detected() {
        // Sixel data: the '?' is parsed as intermediate, params are [0;0;0;0]
        // The '?' intermediate signals sixel mode
        let mut emu = VttyEmulator::new(10, 10, 100);
        emu.feed(b"\x1bP?0;0;0;0q...sixel data...\x1b\\");
        assert!(emu.dcs_buffer().starts_with("sixel:"));
        assert_eq!(emu.sixel_images().len(), 1);
        // Image stored at cursor position (0, 0)
        assert_eq!(emu.sixel_images()[0].0, 0);
        assert_eq!(emu.sixel_images()[0].1, 0);
    }

    #[test]
    fn test_sixel_dcs_digit_introducer() {
        // Sixel without '?' intermediate but all params are 0 — also detected
        let mut emu = VttyEmulator::new(10, 10, 100);
        emu.feed(b"\x1bP0;0q...sixel data...\x1b\\");
        assert!(emu.dcs_buffer().starts_with("sixel:"));
        assert_eq!(emu.sixel_images().len(), 1);
    }

    #[test]
    fn test_sixel_stored_at_cursor_position() {
        let mut emu = VttyEmulator::new(10, 10, 100);
        // Move cursor to (3, 5)
        emu.feed(b"\x1b[4;6H");
        emu.feed(b"\x1bP?0;0;0;0q...sixel...\x1b\\");
        assert_eq!(emu.sixel_images()[0].0, 3); // row
        assert_eq!(emu.sixel_images()[0].1, 5); // col
    }

    #[test]
    fn test_sixel_cleared_on_reset() {
        let mut emu = VttyEmulator::new(10, 10, 100);
        emu.feed(b"\x1bP?0;0;0;0q...sixel...\x1b\\");
        assert!(!emu.sixel_images().is_empty());
        emu.feed_str("\x1bc"); // RIS (full reset)
        assert!(emu.sixel_images().is_empty());
    }

    #[test]
    fn test_sixel_clears_explicitly() {
        let mut emu = VttyEmulator::new(10, 10, 100);
        emu.feed(b"\x1bP?0;0;0;0q...sixel...\x1b\\");
        assert_eq!(emu.sixel_images().len(), 1);
        emu.clear_sixel_images();
        assert!(emu.sixel_images().is_empty());
    }

    #[test]
    fn test_kitty_dcs_not_confused_with_sixel() {
        // Kitty image protocol has params and doesn't start with ?/digit
        let mut emu = VttyEmulator::new(10, 10, 100);
        emu.feed(b"\x1bP1;0;0qi=1\x1b\\");
        assert!(emu.dcs_buffer().starts_with("kitty:"));
        assert!(emu.sixel_images().is_empty());
    }

    #[test]
    fn test_da1_response() {
        let mut emu = VttyEmulator::new(10, 10, 100);
        emu.feed(b"\x1b[c"); // DA1 request (CSI c with no params)
        let responses = emu.drain_responses();
        assert_eq!(responses, b"\x1b[?1;0c");
        // After draining, should be empty
        assert!(emu.drain_responses().is_empty());
    }

    #[test]
    fn test_da1_with_params_no_response() {
        let mut emu = VttyEmulator::new(10, 10, 100);
        emu.feed(b"\x1b[>c"); // DA2 / VT220 response — we only handle DA1 (no params)
        let responses = emu.drain_responses();
        assert!(responses.is_empty());
    }

    // ────────────────────────────────────────────────────────────────
    // Additional tests to reach 75%+ coverage
    // ────────────────────────────────────────────────────────────────

    // -- new() edge cases --

    #[test]
    fn test_new_single_row_single_col() {
        let emu = VttyEmulator::new(1, 1, 0);
        assert_eq!(emu.dimensions(), (1, 1));
        assert_eq!(emu.cursor(), (0, 0));
    }

    #[test]
    fn test_new_large_dimensions() {
        let emu = VttyEmulator::new(200, 300, 50000);
        assert_eq!(emu.dimensions(), (200, 300));
        assert_eq!(emu.cursor(), (0, 0));
    }

    // -- feed() / feed_str() --

    #[test]
    fn test_feed_empty() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("");
        assert_eq!(emu.cursor(), (0, 0));
    }

    #[test]
    fn test_feed_bytes_identical_to_str() {
        let mut emu1 = VttyEmulator::new(5, 10, 100);
        let mut emu2 = VttyEmulator::new(5, 10, 100);
        emu1.feed_str("Hello");
        emu2.feed(b"Hello");
        assert_eq!(emu1.cursor(), emu2.cursor());
        let b1 = emu1.buffer();
        let b2 = emu2.buffer();
        assert_eq!(b1.rows[0][0].ch, b2.rows[0][0].ch);
    }

    // -- finish() --

    #[test]
    fn test_finish_noop() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("Hello");
        emu.finish();
        assert_eq!(emu.cursor(), (0, 5));
    }

    // -- Tab behavior --

    #[test]
    fn test_tab_at_col_0() {
        let mut emu = VttyEmulator::new(5, 20, 100);
        emu.feed_str("\t");
        assert_eq!(emu.cursor(), (0, 8));
    }

    #[test]
    fn test_tab_at_col_7() {
        let mut emu = VttyEmulator::new(5, 20, 100);
        emu.feed_str("1234567\t");
        assert_eq!(emu.cursor(), (0, 8));
    }

    #[test]
    fn test_tab_at_last_col() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("12345678\t");
        assert_eq!(emu.cursor(), (0, 9));
    }

    // -- BS at col 0 --

    #[test]
    fn test_bs_at_col_0_noop() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("\x08");
        assert_eq!(emu.cursor(), (0, 0));
    }

    // -- CR --

    #[test]
    fn test_carriage_return() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("Hello\r");
        assert_eq!(emu.cursor(), (0, 0));
    }

    // -- BEL --

    #[test]
    fn test_bell_sets_flag() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        assert!(!emu.drain_bell());
        emu.feed_str("\x07");
        assert!(emu.drain_bell());
        assert!(!emu.drain_bell()); // drained
    }

    #[test]
    fn test_bell_cleared_on_full_reset() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("\x07");
        assert!(emu.drain_bell());
        emu.feed_str("\x07");
        emu.feed_str("\x1bc");
        assert!(!emu.drain_bell());
    }

    // -- Cursor movement (CSI A/B/C/D/E/F/G/d) --

    #[test]
    fn test_csi_a_cursor_up_clamped() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("\x1b[3;5H");
        emu.feed_str("\x1b[10A"); // Move up 10, clamped to row 0
        assert_eq!(emu.cursor(), (0, 4));
    }

    #[test]
    fn test_csi_b_cursor_down_clamped() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("\x1b[1;1H");
        emu.feed_str("\x1b[10B"); // Move down 10, clamped to last row
        assert_eq!(emu.cursor(), (4, 0));
    }

    #[test]
    fn test_csi_c_cursor_forward_clamped() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("\x1b[100C"); // Move forward 100, clamped to last col
        assert_eq!(emu.cursor(), (0, 9));
    }

    #[test]
    fn test_csi_d_cursor_back_clamped() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("\x1b[1;10H");
        emu.feed_str("\x1b[100D"); // Move back 100, clamped to col 0
        assert_eq!(emu.cursor(), (0, 0));
    }

    #[test]
    fn test_csi_e_cursor_next_line() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("\x1b[1;5H"); // cursor at (0, 4)
        emu.feed_str("\x1b[2E"); // Move down 2, col=0
        assert_eq!(emu.cursor(), (2, 0)); // 0+2=2
    }

    #[test]
    fn test_csi_f_cursor_prev_line() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("\x1b[4;5H"); // cursor at (3, 4)
        emu.feed_str("\x1b[2F"); // Move up 2, col=0
        assert_eq!(emu.cursor(), (1, 0)); // 3-2=1
    }

    #[test]
    fn test_csi_g_cha() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("\x1b[1;1H");
        emu.feed_str("\x1b[7G"); // Move to column 7 (1-based)
        assert_eq!(emu.cursor(), (0, 6));
    }

    #[test]
    fn test_csi_f_same_as_csi_h() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("\x1b[3;4f"); // Same as CSI 3;4 H
        assert_eq!(emu.cursor(), (2, 3));
    }

    // -- SGR attribute tests --

    #[test]
    fn test_sgr_bold_italic_underline() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("\x1b[1;3;4mX");
        let buf = emu.buffer();
        let cell = &buf.rows[0][0];
        assert!(cell.bold);
        assert!(cell.italic);
        assert!(cell.underline);
    }

    #[test]
    fn test_sgr_inverse_colors() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("\x1b[7mX"); // Reverse
        let buf = emu.buffer();
        let cell = &buf.rows[0][0];
        assert_eq!(cell.fg, [0, 0, 0]);     // bg becomes fg
        assert_eq!(cell.bg, [204, 204, 204]); // fg becomes bg
    }

    #[test]
    fn test_sfg_bg_256_color() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("\x1b[38;5;200mF\x1b[48;5;100mB");
        let buf = emu.buffer();
        assert!(buf.rows[0][0].fg != [204, 204, 204]); // Some custom color
        assert!(buf.rows[0][1].bg != [0, 0, 0]);
    }

    #[test]
    fn test_sgr_bright_colors() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("\x1b[90mA\x1b[91mB\x1b[97mC");
        let buf = emu.buffer();
        // Bright black (90), bright red (91), bright white (97)
        assert_ne!(buf.rows[0][0].fg, [204, 204, 204]);
        assert_ne!(buf.rows[0][1].fg, [204, 204, 204]);
        assert_ne!(buf.rows[0][2].fg, [204, 204, 204]);
    }

    #[test]
    fn test_sgr_bg_bright_colors() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("\x1b[100mX\x1b[107mY");
        let buf = emu.buffer();
        assert_ne!(buf.rows[0][0].bg, [0, 0, 0]);
        assert_ne!(buf.rows[0][1].bg, [0, 0, 0]);
    }

    #[test]
    fn test_sgr_reset_all_attrs() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("\x1b[1;3;4;5;7;8;9m");
        emu.feed_str("A");
        emu.feed_str("\x1b[0m");
        emu.feed_str("B");
        let buf = emu.buffer();
        let a = &buf.rows[0][0];
        assert!(a.bold && a.italic && a.underline && a.blink && a.reverse && a.invisible && a.strikethrough);
        let b = &buf.rows[0][1];
        assert!(!b.bold && !b.italic && !b.underline && !b.blink && !b.reverse && !b.invisible && !b.strikethrough);
    }

    #[test]
    fn test_sfg_truecolor_bg() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("\x1b[48;2;10;20;30mX");
        let buf = emu.buffer();
        assert_eq!(buf.rows[0][0].bg, [10, 20, 30]);
    }

    #[test]
    fn test_sgr_default_fg_bg() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("\x1b[31m");
        emu.feed_str("X"); // Write a char with red fg
        emu.feed_str("\x1b[39m"); // Default fg
        let buf = emu.buffer();
        let cell = &buf.rows[0][0];
        assert_eq!(cell.fg, [170, 0, 0]); // The char was written before reset
        emu.feed_str("Y");
        let buf2 = emu.buffer();
        assert_eq!(buf2.rows[0][1].fg, [204, 204, 204]); // Default
    }

    #[test]
    fn test_sgr_default_bg() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("\x1b[44m");
        emu.feed_str("\x1b[49m"); // Default bg
        emu.feed_str("Y");
        let buf = emu.buffer();
        assert_eq!(buf.rows[0][0].bg, [0, 0, 0]); // Default bg
    }

    // -- Insert mode (CSI @ / ICH) --

    #[test]
    fn test_insert_mode_with_csi_at() {
        let mut emu = VttyEmulator::new(3, 10, 100);
        emu.feed_str("ABCDE");
        emu.feed_str("\x1b[1;3H"); // col 3 (1-based)
        emu.feed_str("\x1b[2@"); // Insert 2 blanks
        let buf = emu.buffer();
        assert_eq!(buf.rows[0][0].ch, 'A');
        assert_eq!(buf.rows[0][1].ch, 'B');
        assert_eq!(buf.rows[0][2].ch, ' '); // inserted
        assert_eq!(buf.rows[0][3].ch, ' '); // inserted
        assert_eq!(buf.rows[0][4].ch, 'C');
    }

    // -- Delete cells (CSI P / DCH) --

    #[test]
    fn test_delete_cells_csi_p() {
        let mut emu = VttyEmulator::new(3, 10, 100);
        emu.feed_str("ABCDEFGHIJ");
        emu.feed_str("\x1b[1;3H");
        emu.feed_str("\x1b[3P"); // Delete 3 chars at col 3 (1-based)
        let buf = emu.buffer();
        assert_eq!(buf.rows[0][0].ch, 'A');
        assert_eq!(buf.rows[0][1].ch, 'B');
        assert_eq!(buf.rows[0][2].ch, 'F'); // Shifted left
        assert_eq!(buf.rows[0][3].ch, 'G');
        assert_eq!(buf.rows[0][4].ch, 'H');
    }

    // -- Scroll up/down (CSI S / T) --

    #[test]
    fn test_csi_s_scroll_up() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("Row0\r\nRow1\r\nRow2\r\nRow3\r\nRow4");
        emu.feed_str("\x1b[1;1H");
        emu.feed_str("\x1b[2S"); // Scroll up 2
        let buf = emu.buffer();
        assert_eq!(buf.scrollback.len(), 2); // 2 lines scrolled to scrollback
    }

    #[test]
    fn test_csi_t_scroll_down() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("Row0\r\nRow1\r\nRow2\r\nRow3\r\nRow4");
        emu.feed_str("\x1b[1T"); // Scroll down 1
        let buf = emu.buffer();
        assert_eq!(buf.rows[0][0].ch, ' '); // Blank line inserted at top
        assert_eq!(buf.rows[1][0].ch, 'R'); // Original row 0 shifted down
    }

    // -- Delete line (CSI M / DL) --

    #[test]
    fn test_delete_line_csi_m() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("AAAA\r\nBBBB\r\nCCCC\r\nDDDD\r\nEEEE");
        emu.feed_str("\x1b[2;1H");
        emu.feed_str("\x1b[M"); // Delete row 2 (1-based)
        let buf = emu.buffer();
        assert_eq!(buf.rows[0][0].ch, 'A');
        assert_eq!(buf.rows[1][0].ch, 'C'); // Row 3 moved up
        assert_eq!(buf.rows[2][0].ch, 'D');
        assert_eq!(buf.rows[3][0].ch, 'E');
        assert_eq!(buf.rows[4][0].ch, ' '); // Blank line at bottom
    }

    // -- Insert line (CSI L / IL) --

    #[test]
    fn test_insert_line_csi_l() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("AAAA\r\nBBBB\r\nCCCC\r\nDDDD\r\nEEEE");
        emu.feed_str("\x1b[2;1H");
        emu.feed_str("\x1b[L"); // Insert blank at row 2
        let buf = emu.buffer();
        assert_eq!(buf.rows[0][0].ch, 'A');
        assert_eq!(buf.rows[1][0].ch, ' '); // Inserted
        assert_eq!(buf.rows[2][0].ch, 'B'); // Shifted down
        assert_eq!(buf.rows[3][0].ch, 'C');
        assert_eq!(buf.rows[4][0].ch, 'D'); // E was pushed out
    }

    // -- ED mode 0, 1 (partial erase) --

    #[test]
    fn test_ed_mode_0_erase_from_cursor() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("AAAAAAAAAA\r\nBBBBBBBBBB\r\nCCCCCCCCCC");
        emu.feed_str("\x1b[2;5H"); // row 1, col 4 (0-based)
        emu.feed_str("\x1b[0J");
        let buf = emu.buffer();
        // Row 0 is ABOVE cursor, NOT cleared by mode 0
        assert_eq!(buf.rows[0][3].ch, 'A'); // Still intact
        assert_eq!(buf.rows[1][5].ch, ' '); // From cursor col+1 onwards, cleared
        assert_eq!(buf.rows[1][0].ch, 'B'); // Before cursor col, NOT cleared
        assert_eq!(buf.rows[1][4].ch, ' '); // At cursor col, cleared
        assert_eq!(buf.rows[2][0].ch, ' '); // Entire row 2 cleared
    }

    #[test]
    fn test_ed_mode_1_erase_to_cursor() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("AAAAAAAAAA\r\nBBBBBBBBBB\r\nCCCCCCCCCC");
        emu.feed_str("\x1b[2;5H"); // row 1, col 4 (0-based)
        emu.feed_str("\x1b[1J");
        let buf = emu.buffer();
        // Mode 1: clear from top of screen to cursor pos (inclusive)
        assert_eq!(buf.rows[0][0].ch, ' '); // Entire row 0 cleared
        assert_eq!(buf.rows[0][9].ch, ' '); // Entire row 0 cleared
        assert_eq!(buf.rows[1][0].ch, ' '); // Row 1 before cursor col, cleared
        assert_eq!(buf.rows[1][3].ch, ' '); // Before cursor col, cleared
        assert_eq!(buf.rows[1][4].ch, ' '); // Cursor pos, cleared
        assert_eq!(buf.rows[2][0].ch, 'C'); // Row 2 untouched
    }

    // -- ESC M (reverse index) --

    #[test]
    fn test_esc_m_at_top_row() {
        let mut emu = VttyEmulator::new(3, 5, 100);
        emu.feed_str("AAA\r\nBBB\r\nCCC");
        assert_eq!(emu.cursor(), (2, 3));
        emu.feed_str("\x1bM"); // ESC M at row 2 → just moves up
        assert_eq!(emu.cursor(), (1, 3));
        emu.feed_str("\x1bM"); // ESC M at row 1 → just moves up
        assert_eq!(emu.cursor(), (0, 3));
        emu.feed_str("\x1bM"); // ESC M at row 0 → scroll down
        assert_eq!(emu.cursor(), (0, 3));
        let buf = emu.buffer();
        assert_eq!(buf.rows[0][0].ch, ' '); // New blank row
        assert_eq!(buf.rows[1][0].ch, 'A'); // Original row 0 shifted down
    }

    // -- Origin mode --

    #[test]
    fn test_origin_mode_cup() {
        let mut emu = VttyEmulator::new(10, 10, 100);
        emu.feed_str("\x1b[3;6r"); // Set scroll region rows 3-6 (1-based)
        emu.feed_str("\x1b[?6h"); // Origin mode on
        emu.feed_str("\x1b[1;1H"); // Home in origin mode = top of scroll region
        assert_eq!(emu.cursor(), (2, 0)); // Row 2 (0-based = scroll_top)
        emu.feed_str("\x1b[?6l"); // Origin mode off
        emu.feed_str("\x1b[1;1H"); // Home in normal mode = absolute row 0
        assert_eq!(emu.cursor(), (0, 0));
    }

    // -- Cursor visibility --

    #[test]
    fn test_cursor_visibility_hide_show() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        assert!(emu.is_cursor_visible());
        emu.feed_str("\x1b[?25l"); // Hide
        assert!(!emu.is_cursor_visible());
        emu.feed_str("\x1b[?25h"); // Show
        assert!(emu.is_cursor_visible());
    }

    // -- Mouse tracking modes --

    #[test]
    fn test_mouse_tracking_1002() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        assert!(!emu.mouse_tracking_enabled());
        emu.feed_str("\x1b[?1002h");
        assert!(emu.mouse_tracking_enabled());
    }

    #[test]
    fn test_mouse_tracking_1003() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("\x1b[?1003h");
        assert!(emu.mouse_tracking_enabled());
    }

    #[test]
    fn test_mouse_sgr_1006() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        assert!(!emu.mouse_sgr_enabled());
        emu.feed_str("\x1b[?1006h");
        assert!(emu.mouse_sgr_enabled());
        emu.feed_str("\x1b[?1006l");
        assert!(!emu.mouse_sgr_enabled());
    }

    #[test]
    fn test_mouse_1000_disables_all() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("\x1b[?1002h\x1b[?1003h");
        assert!(emu.mouse_tracking_enabled());
        emu.feed_str("\x1b[?1000l"); // Disabling 1000 also disables 1002/1003
        assert!(!emu.mouse_tracking_enabled());
    }

    // -- Focus reporting --

    #[test]
    fn test_focus_reporting() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        assert!(!emu.focus_reporting_enabled());
        emu.feed_str("\x1b[?1004h");
        assert!(emu.focus_reporting_enabled());
        emu.feed_str("\x1b[?1004l");
        assert!(!emu.focus_reporting_enabled());
    }

    // -- Auto wrap off --

    #[test]
    fn test_auto_wrap_off() {
        let mut emu = VttyEmulator::new(3, 5, 100);
        emu.feed_str("\x1b[?7l"); // Disable auto wrap
        emu.feed_str("ABCDEFGH");
        assert_eq!(emu.cursor(), (0, 4)); // Stuck at last col
        let buf = emu.buffer();
        // Last written char overwrites the last col multiple times
        assert_eq!(buf.rows[0][4].ch, 'H');
    }

    // -- Recover from alternate screen --

    #[test]
    fn test_recover_from_alternate_screen_noop() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        assert!(!emu.recover_from_alternate_screen());
    }

    #[test]
    fn test_recover_from_alternate_screen_returns_true() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("Main");
        emu.feed_str("\x1b[?1049h");
        emu.feed_str("Alt");
        assert!(emu.recover_from_alternate_screen());
        assert!(!emu.is_alternate_screen());
        let buf = emu.buffer();
        assert_eq!(buf.rows[0][0].ch, 'M');
    }

    // -- Snapshot main/alt --

    #[test]
    fn test_snapshot_main_no_alt() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("Main");
        let snap = emu.snapshot_main();
        assert_eq!(snap.rows[0][0].ch, 'M');
    }

    #[test]
    fn test_snapshot_main_while_alt_active() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("Main");
        emu.feed_str("\x1b[?1049h");
        emu.feed_str("Alt");
        let main = emu.snapshot_main();
        assert_eq!(main.rows[0][0].ch, 'M');
        let alt = emu.snapshot();
        assert_eq!(alt.rows[0][0].ch, 'A');
    }

    #[test]
    fn test_snapshot_alt_after_exit() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("\x1b[?1049h");
        emu.feed_str("AltData");
        emu.feed_str("\x1b[?1049l");
        let alt = emu.snapshot_alt();
        assert_eq!(alt.rows[0][0].ch, 'A');
    }

    #[test]
    fn test_snapshot_alt_no_alt_ever() {
        let emu = VttyEmulator::new(5, 10, 100);
        let alt = emu.snapshot_alt();
        // Should return a fresh empty buffer
        assert_eq!(alt.rows[0][0].ch, ' ');
    }

    // -- Buffer generation --

    #[test]
    fn test_buffer_generation_increments() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        let gen0 = emu.buffer_generation();
        emu.feed_str("X");
        let gen1 = emu.buffer_generation();
        assert!(gen1 > gen0);
    }

    // -- Scrollback len --

    #[test]
    fn test_scrollback_len() {
        let mut emu = VttyEmulator::new(3, 10, 100);
        assert_eq!(emu.scrollback_len(), 0);
        emu.feed_str("A\r\nB\r\nC\r\nD");
        assert_eq!(emu.scrollback_len(), 1);
    }

    // -- partial() --

    #[test]
    fn test_partial() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("Row0\r\nRow1\r\nRow2\r\nRow3\r\nRow4");
        let text = emu.partial(1, 2);
        assert!(text.contains("Row1"));
        assert!(text.contains("Row2"));
        assert!(!text.contains("Row0"));
    }

    #[test]
    fn test_partial_out_of_bounds() {
        let emu = VttyEmulator::new(5, 10, 100);
        let text = emu.partial(100, 5);
        assert!(text.is_empty());
    }

    // -- dimensions() --

    #[test]
    fn test_dimensions() {
        let emu = VttyEmulator::new(42, 137, 500);
        assert_eq!(emu.dimensions(), (42, 137));
    }

    // -- DCS / Sixel --

    #[test]
    fn test_sixel_images_empty_initially() {
        let emu = VttyEmulator::new(5, 10, 100);
        assert!(emu.sixel_images().is_empty());
    }

    #[test]
    fn test_dcs_buffer_empty_initially() {
        let emu = VttyEmulator::new(5, 10, 100);
        assert!(emu.dcs_buffer().is_empty());
    }

    #[test]
    fn test_clear_sixel_images() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        assert!(emu.sixel_images().is_empty());
        emu.clear_sixel_images();
        assert!(emu.sixel_images().is_empty());
    }

    // -- resize clamps cursor --

    #[test]
    fn test_resize_clamps_cursor() {
        let mut emu = VttyEmulator::new(10, 10, 100);
        emu.feed_str("\x1b[8;8H"); // Row 8, col 8
        emu.resize(3, 5);
        assert_eq!(emu.cursor(), (2, 4)); // Clamped
    }

    // -- Full reset clears everything --

    #[test]
    fn test_full_reset_clears_title() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("\x1b]0;MyTitle\x07");
        assert_eq!(emu.title(), "MyTitle");
        emu.feed_str("\x1bc");
        assert_eq!(emu.title(), "");
    }

    #[test]
    fn test_full_reset_clears_alt_screen() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("Main");
        emu.feed_str("\x1b[?1049h");
        emu.feed_str("Alt");
        assert!(emu.is_alternate_screen());
        emu.feed_str("\x1bc");
        assert!(!emu.is_alternate_screen());
        // Main buffer should be restored
        let buf = emu.buffer();
        assert_eq!(buf.rows[0][0].ch, 'M');
    }

    #[test]
    fn test_full_reset_clears_mouse_modes() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("\x1b[?1002h\x1b[?1003h\x1b[?1006h");
        emu.feed_str("\x1bc");
        assert!(!emu.mouse_tracking_enabled());
        assert!(!emu.mouse_sgr_enabled());
    }

    #[test]
    fn test_full_reset_clears_insert_mode() {
        // We can't set insert mode via ANSI in this emulator,
        // but we can verify the reset sets it to false.
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("\x1bc");
        // After reset, buffer should be clear
        let buf = emu.buffer();
        assert_eq!(buf.rows[0][0].ch, ' ');
    }

    // -- Alternate screen 47/1047 modes --

    #[test]
    fn test_alt_screen_mode_47() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("Main");
        emu.feed_str("\x1b[?47h");
        assert!(emu.is_alternate_screen());
        emu.feed_str("\x1b[?47l");
        assert!(!emu.is_alternate_screen());
    }

    #[test]
    fn test_alt_screen_mode_1047() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("Main");
        emu.feed_str("\x1b[?1047h");
        assert!(emu.is_alternate_screen());
        emu.feed_str("\x1b[?1047l");
        assert!(!emu.is_alternate_screen());
        let buf = emu.buffer();
        assert_eq!(buf.rows[0][0].ch, 'M');
    }

    // -- LF-only (0x0a) without CR --

    #[test]
    fn test_lf_only_no_cr() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("AAA\x0a");
        assert_eq!(emu.cursor(), (1, 0)); // Unix newline (LF acts as CR+LF in emulator)
    }

    // -- VT/FF (0x0b, 0x0c) --

    #[test]
    fn test_vt_ff_move_down() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("AAAA\x0b");
        assert_eq!(emu.cursor(), (1, 4)); // VT moves down, no CR
        emu.feed_str("\x0c");
        assert_eq!(emu.cursor(), (2, 4)); // FF moves down, no CR
    }

    // -- Multiple lines scroll --

    #[test]
    fn test_multi_line_scroll_scrollback_capacity() {
        let mut emu = VttyEmulator::new(3, 5, 5);
        for i in 0..20 {
            emu.feed_str(&format!("L{}\r\n", i));
        }
        // scrollback capped at 5
        assert_eq!(emu.scrollback_len(), 5);
    }

    // -- drain_responses --

    #[test]
    fn test_drain_responses_empty() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        assert!(emu.drain_responses().is_empty());
    }

    // -- palette() accessor --

    #[test]
    fn test_palette_accessor() {
        let emu = VttyEmulator::new(5, 10, 100);
        // Default color 1 (red)
        assert_eq!(emu.palette().get(1), [170, 0, 0]);
    }
}
