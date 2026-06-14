use super::{
    buffer::Buffer,
    cell::{char_width, Cell},
    color::ColorPalette,
};
use parking_lot::RwLock;
use std::sync::Arc;
use vte::Perform;

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

/// Terminal mode flags, grouped for clarity and shared reset.
#[derive(Debug, Clone, Copy)]
struct TerminalModes {
    auto_wrap: bool,
    insert_mode: bool,
    origin_mode: bool,
    cursor_visible: bool,
    alternate_screen: bool,
    mouse_sgr: bool,
    mouse_button_tracking: bool,
    mouse_any_tracking: bool,
    bracketed_paste: bool,
    focus_reporting: bool,
}

impl Default for TerminalModes {
    fn default() -> Self {
        Self {
            auto_wrap: true,
            insert_mode: false,
            origin_mode: false,
            cursor_visible: true,
            alternate_screen: false,
            mouse_sgr: false,
            mouse_button_tracking: false,
            mouse_any_tracking: false,
            bracketed_paste: false,
            focus_reporting: false,
        }
    }
}

impl TerminalModes {
    fn reset(&mut self) {
        *self = Self::default();
    }
}

#[derive(Debug, Clone, Copy)]
struct SavedCursor {
    row: usize,
    col: usize,
    attrs: Cell,
}

pub struct VttyEmulator {
    buffer: Arc<RwLock<Buffer>>,
    cursor_row: usize,
    cursor_col: usize,
    /// Current text attributes. Only the style fields (fg, bg, bold, etc.)
    /// are meaningful; `ch` and `width` are ignored for attribute purposes.
    attrs: Cell,
    saved_cursor: Option<SavedCursor>,
    parser: vte::Parser,
    modes: TerminalModes,
    scroll_top: usize,
    scroll_bottom: usize,
    /// When true, the cursor is at the last column and a wrap to the
    /// next line is pending.  The wrap is executed when the next
    /// printable character is written (VT100 deferred-wrap semantics).
    wrap_pending: bool,
    /// Current window title set via OSC 0 / OSC 2.
    title: String,
    /// Pending bell flag — set when BEL (0x07) is received.
    /// Checked and cleared by drain_bell().
    bell_pending: bool,
    /// Current cursor style (DECSCUSR).
    cursor_style: CursorStyle,
    /// Most recent DCS data string (e.g. kitty graphics protocol payload).
    dcs_buffer_str: String,
    /// Raw bytes accumulated during DCS sequences (hook/put/unhook).
    dcs_raw: Vec<u8>,
    /// DCS intermediates bytes, stored during hook() for use in unhook().
    dcs_intermediates: Vec<u8>,
    /// DCS final byte, stored during hook() for use in unhook().
    dcs_final: char,
    /// Pending response bytes to send back to the child PTY.
    /// Collected during feed() and consumed by drain_responses().
    response_buf: Vec<u8>,
    /// Mutable 256-color palette, modifiable at runtime via OSC 4.
    palette: ColorPalette,
    /// Stored inline images from Sixel DCS sequences.
    /// Each entry is (row, col, sixel_data_string).
    sixel_images: Vec<(usize, usize, String)>,
    max_scrollback: usize,
    cols: usize,
    rows: usize,
    main_buffer: Option<Buffer>,
    alt_buffer: Option<Buffer>,
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
        let emu = Self {
            buffer,
            cursor_row: 0,
            cursor_col: 0,
            attrs: Cell::default(),
            saved_cursor: None,
            parser: vte::Parser::new(),
            modes: TerminalModes::default(),
            scroll_top: 0,
            scroll_bottom: rows.saturating_sub(1),
            wrap_pending: false,
            title: String::new(),
            bell_pending: false,
            cursor_style: CursorStyle::Block(true),
            dcs_buffer_str: String::new(),
            dcs_raw: Vec::new(),
            dcs_intermediates: Vec::new(),
            dcs_final: '\0',
            response_buf: Vec::new(),
            palette: ColorPalette::new(),
            sixel_images: Vec::new(),
            max_scrollback,
            cols,
            rows,
            main_buffer: None,
            alt_buffer: None,
        };
        emu
    }

    /// Shared reset logic called by both `new()` and `full_reset()`.
    fn reset_state(&mut self) {
        self.cursor_row = 0;
        self.cursor_col = 0;
        self.attrs = Cell::default();
        self.saved_cursor = None;
        self.modes.reset();
        self.scroll_top = 0;
        self.scroll_bottom = self.rows.saturating_sub(1);
        self.wrap_pending = false;
        self.title.clear();
        self.bell_pending = false;
        self.cursor_style = CursorStyle::Block(true);
        self.dcs_buffer_str.clear();
        self.dcs_raw.clear();
        self.dcs_intermediates.clear();
        self.dcs_final = '\0';
        self.response_buf.clear();
        self.sixel_images.clear();
        self.palette.reset();
        {
            let mut buf = self.buffer.write();
            buf.clear_all(None);
        }
        if self.modes.alternate_screen {
            self.exit_alternate_screen();
        }
        self.alt_buffer = None;
    }

    pub fn feed(&mut self, data: &[u8]) {
        // vte::Parser::advance(&mut self, &mut Perform) borches both
        // self.parser and self mutably — split them to satisfy the borrow checker.
        let mut parser = std::mem::take(&mut self.parser);
        parser.advance(self, data);
        self.parser = parser;
    }

    pub fn feed_str(&mut self, s: &str) {
        self.feed(s.as_bytes());
    }

    // ────────────────────────────────────────────────────────────────
    // Print / character output
    // ────────────────────────────────────────────────────────────────

    fn write_char(&mut self, ch: char) {
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
            if self.modes.auto_wrap {
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
            if self.modes.insert_mode {
                buf.insert_cells(self.cursor_row, self.cursor_col, cw as usize, None);
            }
            if self.cursor_row < self.rows && self.cursor_col < self.cols {
                // Apply reverse video if set on attrs.
                let (fg, bg) = if self.attrs.reverse {
                    (self.attrs.bg, self.attrs.fg)
                } else {
                    (self.attrs.fg, self.attrs.bg)
                };
                let cell = Cell {
                    ch,
                    fg,
                    bg,
                    width: cw,
                    ..self.attrs
                };
                buf.set(self.cursor_row, self.cursor_col, cell);

                // Place a continuation cell for wide characters.
                if cw == 2 && self.cursor_col + 1 < self.cols {
                    let cont = Cell::continuation_of(&self.attrs);
                    buf.set(self.cursor_row, self.cursor_col + 1, cont);
                }
            }
        } // Drop the write guard before touching self again

        // Advance cursor.
        self.cursor_col += cw as usize;
        if self.cursor_col >= self.cols {
            if self.modes.auto_wrap {
                self.wrap_pending = true;
                // Keep cursor_col at cols-1 (the last column) visually;
                // the pending flag ensures the next char wraps.
                self.cursor_col = self.cols.saturating_sub(1);
            } else {
                self.cursor_col = self.cols.saturating_sub(1);
            }
        }
    }

    // ────────────────────────────────────────────────────────────────
    // CSI dispatch
    // ────────────────────────────────────────────────────────────────

    fn process_csi(
        &mut self,
        params: &vte::Params,
        intermediates: &[u8],
        _ignore: bool,
        final_byte: char,
    ) {
        // Collect params into a flat Vec<u16> for efficient random access.
        // vte separates subparams with `:` but standard ECMA-48 SGR uses
        // only `;` so each param slice has exactly one element.
        let p: Vec<u16> = params.iter().map(|s| s.first().copied().unwrap_or(0)).collect();

        // ECMA-48: a parameter value of 0 or missing means "use default".
        let param = |idx: usize, default: u16| -> u16 {
            p.get(idx)
                .map(|v| if *v == 0 { default } else { *v })
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
            'H' | 'f' => {
                clear_wrap();
                let row = param_1based(0, 1);
                let col = param_1based(1, 1);
                if self.modes.origin_mode {
                    self.cursor_row = (self.scroll_top + row).min(self.scroll_bottom);
                } else {
                    self.cursor_row = row.min(self.rows.saturating_sub(1));
                }
                self.cursor_col = col.min(self.cols.saturating_sub(1));
            }
            'A' => {
                clear_wrap();
                let n = param(0, 1) as usize;
                self.cursor_row = self.cursor_row.saturating_sub(n);
            }
            'B' => {
                clear_wrap();
                let n = param(0, 1) as usize;
                self.cursor_row = (self.cursor_row + n).min(self.rows.saturating_sub(1));
            }
            'C' => {
                clear_wrap();
                let n = param(0, 1) as usize;
                self.cursor_col = (self.cursor_col + n).min(self.cols.saturating_sub(1));
            }
            'D' => {
                clear_wrap();
                let n = param(0, 1) as usize;
                self.cursor_col = self.cursor_col.saturating_sub(n);
            }
            'E' => {
                clear_wrap();
                let n = param(0, 1) as usize;
                self.cursor_row = (self.cursor_row + n).min(self.rows.saturating_sub(1));
                self.cursor_col = 0;
            }
            'F' => {
                clear_wrap();
                let n = param(0, 1) as usize;
                self.cursor_row = self.cursor_row.saturating_sub(n);
                self.cursor_col = 0;
            }
            'G' => {
                clear_wrap();
                let col = param_1based(0, 1);
                self.cursor_col = col.min(self.cols.saturating_sub(1));
            }
            'd' => {
                // CSI n d — Vertical Position Absolute (VPA)
                clear_wrap();
                let row = param_1based(0, 1);
                if self.modes.origin_mode {
                    self.cursor_row = (self.scroll_top + row).min(self.scroll_bottom);
                } else {
                    self.cursor_row = row.min(self.rows.saturating_sub(1));
                }
            }
            'X' => {
                // CSI n X — Erase Characters (ECH)
                clear_wrap();
                let n = param(0, 1) as usize;
                let blank = self.make_blank();
                let mut buf = self.buffer.write();
                let mut erased = 0;
                let mut col = self.cursor_col;
                while erased < n && col < self.cols {
                    buf.set(self.cursor_row, col, blank);
                    col += 1;
                    erased += 1;
                }
            }
            'J' => {
                clear_wrap();
                let mode = param(0, 0);
                let template = Some(&self.make_blank());
                let mut buf = self.buffer.write();
                match mode {
                    0 => buf.clear_screen_from(self.cursor_row, self.cursor_col, template),
                    1 => buf.clear_screen_to(self.cursor_row, self.cursor_col, template),
                    2 | 3 => buf.clear_all(template),
                    _ => {}
                }
            }
            'K' => {
                clear_wrap();
                let mode = param(0, 0);
                let template = Some(&self.make_blank());
                let mut buf = self.buffer.write();
                match mode {
                    0 => buf.clear_line_from(self.cursor_row, self.cursor_col, template),
                    1 => buf.clear_line_to(self.cursor_row, self.cursor_col, template),
                    2 => buf.clear_line(self.cursor_row, template),
                    _ => {}
                }
            }
            'L' => {
                clear_wrap();
                let n = param(0, 1) as usize;
                let template = Some(&self.make_blank());
                let mut buf = self.buffer.write();
                for _ in 0..n {
                    buf.insert_line(self.cursor_row, Some(self.scroll_bottom), template);
                }
            }
            'M' => {
                clear_wrap();
                let n = param(0, 1) as usize;
                let template = Some(&self.make_blank());
                let mut buf = self.buffer.write();
                for _ in 0..n {
                    buf.delete_line(self.cursor_row, Some(self.scroll_bottom), template);
                }
            }
            'P' => {
                clear_wrap();
                let n = param(0, 1) as usize;
                let template = Some(&self.make_blank());
                let mut buf = self.buffer.write();
                buf.delete_cells(self.cursor_row, self.cursor_col, n, template);
            }
            '@' => {
                clear_wrap();
                let n = param(0, 1) as usize;
                let template = Some(&self.make_blank());
                let mut buf = self.buffer.write();
                buf.insert_cells(self.cursor_row, self.cursor_col, n, template);
            }
            'S' => {
                clear_wrap();
                let n = param(0, 1) as usize;
                let template = Some(&self.make_blank());
                let mut buf = self.buffer.write();
                for _ in 0..n {
                    buf.scroll_region_up(self.scroll_top, self.scroll_bottom, template);
                }
            }
            'T' => {
                clear_wrap();
                let n = param(0, 1) as usize;
                let template = Some(&self.make_blank());
                let mut buf = self.buffer.write();
                for _ in 0..n {
                    buf.scroll_region_down(self.scroll_top, self.scroll_bottom, template);
                }
            }
            'r' => {
                // CSI Pt;Pb r — Set Scrolling Region
                self.scroll_top = param_1based(0, 1);
                self.scroll_bottom = if p.len() > 1 {
                    param_1based(1, self.rows)
                } else {
                    self.rows.saturating_sub(1)
                };
                if self.modes.origin_mode {
                    self.cursor_row = self.scroll_top;
                } else {
                    self.cursor_row = 0;
                }
                self.cursor_col = 0;
                self.wrap_pending = false;
            }
            's' => {
                // CSI s — Save cursor position (ANSI.SYS)
                self.saved_cursor = Some(SavedCursor {
                    row: self.cursor_row,
                    col: self.cursor_col,
                    attrs: self.attrs,
                });
            }
            'u' => {
                // CSI u — Restore cursor position (ANSI.SYS)
                if let Some(saved) = self.saved_cursor {
                    self.cursor_row = saved.row.min(self.rows.saturating_sub(1));
                    self.cursor_col = saved.col.min(self.cols.saturating_sub(1));
                    self.attrs = saved.attrs;
                }
            }
            'm' => {
                self.process_sgr(&p);
            }
            'h' | 'l' => {
                let set = final_byte == 'h';
                if intermediates.first() == Some(&b'?') {
                    self.process_dec_private_mode(&p, set);
                } else {
                    // Standard ECMA-48 SM/RM modes
                    for &mode in &p {
                        match mode {
                            4 => self.modes.insert_mode = set,
                            _ => {}
                        }
                    }
                }
            }
            'n' => {
                // DSR — Device Status Report
                if param(0, 0) == 6 {
                    // CPR — Cursor Position Report
                    let row = self.cursor_row + 1;
                    let col = self.cursor_col + 1;
                    let response = format!("\x1b[{};{}R", row, col);
                    self.response_buf.extend_from_slice(response.as_bytes());
                }
            }
            'q' if intermediates == [0x20] => {
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
            'c' if intermediates.is_empty() => {
                self.response_buf.extend_from_slice(b"\x1b[?1;0c");
            }
            _ => {}
        }
    }

    /// Build a blank cell with the current attrs style (applying reverse).
    fn make_blank(&self) -> Cell {
        let (fg, bg) = if self.attrs.reverse {
            (self.attrs.bg, self.attrs.fg)
        } else {
            (self.attrs.fg, self.attrs.bg)
        };
        Cell {
            ch: ' ',
            fg,
            bg,
            width: 1,
            ..self.attrs
        }
    }

    fn process_sgr(&mut self, params: &[u16]) {
        if params.is_empty() {
            self.attrs = Cell::default();
            return;
        }
        let mut i = 0;
        while i < params.len() {
            let param = params[i];
            match param {
                0 => self.attrs = Cell::default(),
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
                    if let Some(rgb) = self.parse_sgr_color(params, &mut i) {
                        self.attrs.fg = rgb;
                    }
                }
                39 => self.attrs.fg = [204, 204, 204],
                40..=47 => {
                    self.attrs.bg = self.palette.resolve(param as u8 - 40);
                }
                48 => {
                    if let Some(rgb) = self.parse_sgr_color(params, &mut i) {
                        self.attrs.bg = rgb;
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

    /// Parse an SGR extended color (256-color or truecolor) starting at params[i+1].
    /// Advances `i` past the consumed parameters. Returns Some(RGB) or None.
    fn parse_sgr_color(&self, params: &[u16], i: &mut usize) -> Option<[u8; 3]> {
        let color_type = params.get(*i + 1).copied().unwrap_or(0);
        match color_type {
            2 => {
                // Truecolor: 38;2;R;G;B or 48;2;R;G;B
                if *i + 4 < params.len() {
                    let r = params[*i + 2] as u8;
                    let g = params[*i + 3] as u8;
                    let b = params[*i + 4] as u8;
                    *i += 4;
                    Some([r, g, b])
                } else {
                    None
                }
            }
            5 => {
                // 256-color: 38;5;N or 48;5;N
                if let Some(&idx) = params.get(*i + 2) {
                    let rgb = self.palette.resolve(idx as u8);
                    *i += 2;
                    Some(rgb)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn process_dec_private_mode(&mut self, params: &[u16], set: bool) {
        for &mode in params {
            match mode {
                25 => self.modes.cursor_visible = set,
                47 | 1047 | 1049 => {
                    if set && !self.modes.alternate_screen {
                        self.enter_alternate_screen();
                    } else if !set && self.modes.alternate_screen {
                        self.exit_alternate_screen();
                    }
                }
                7 => self.modes.auto_wrap = set,
                6 => self.modes.origin_mode = set,
                1000 if !set => {
                    // Disabling 1000 also disables higher modes
                    self.modes.mouse_button_tracking = false;
                    self.modes.mouse_any_tracking = false;
                }
                1002 => self.modes.mouse_button_tracking = set,
                1003 => self.modes.mouse_any_tracking = set,
                1006 => self.modes.mouse_sgr = set,
                1004 => self.modes.focus_reporting = set,
                2004 => self.modes.bracketed_paste = set,
                _ => {}
            }
        }
    }

    // ────────────────────────────────────────────────────────────────
    // OSC handling
    // ────────────────────────────────────────────────────────────────

    fn process_osc(&mut self, osc_params: &[&[u8]]) {
        // vte gives us &[&[u8]] where osc_params[0] is the code and
        // osc_params[1..] are data parts separated by ';'.
        if osc_params.is_empty() {
            return;
        }
        let code = std::str::from_utf8(osc_params[0]).unwrap_or("");
        let data = if osc_params.len() > 1 {
            // Join remaining params with ';'
            let parts: Vec<&str> = osc_params[1..]
                .iter()
                .filter_map(|p| std::str::from_utf8(p).ok())
                .collect();
            parts.join(";")
        } else {
            String::new()
        };

        match code {
            "0" | "2" => self.title = data,
            "4" => {
                self.palette.apply_osc4(&data);
            }
            "777" => {
                // OSC 777 — desktop notification.  Format: "777;title;body"
                let parts: Vec<&str> = data.splitn(2, ';').collect();
                let title = parts.first().copied().unwrap_or("");
                let body = parts.get(1).copied().unwrap_or("");
                tracing::debug!(title, body, "OSC 777 desktop notification");
            }
            "104" => {
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

    // ────────────────────────────────────────────────────────────────
    // DCS handling (Sixel / Kitty graphics)
    // ────────────────────────────────────────────────────────────────

    fn process_dcs(
        &mut self,
        params: &vte::Params,
        intermediates: &[u8],
        final_byte: char,
        data: &[u8],
    ) {
        if final_byte != 'q' {
            // Other DCS sequences are silently consumed.
            return;
        }

        // Collect params for analysis
        let p: Vec<u16> = params.iter().map(|s| s.first().copied().unwrap_or(0)).collect();

        let is_sixel = intermediates.contains(&b'?')
            || (intermediates.is_empty()
                && p.iter().all(|&v| v == 0)
                && !data.is_empty());

        if is_sixel {
            let pos = (self.cursor_row, self.cursor_col);
            let data_str = String::from_utf8_lossy(data).to_string();
            self.dcs_buffer_str = format!("sixel:{}:{}", data.len(), data_str);
            self.sixel_images.push((pos.0, pos.1, data_str));
            tracing::debug!(
                row = pos.0,
                col = pos.1,
                len = data.len(),
                "Stored Sixel inline image"
            );
            return;
        }

        // Kitty image protocol
        let ps = p
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join(";");
        let data_str = String::from_utf8_lossy(data).to_string();
        self.dcs_buffer_str = format!("kitty:{}:{}", ps, data_str);
    }

    // ────────────────────────────────────────────────────────────────
    // Scroll / alternate screen
    // ────────────────────────────────────────────────────────────────

    fn check_scroll(&mut self) {
        if self.cursor_row > self.scroll_bottom {
            let template = Some(&self.make_blank());
            let mut buf = self.buffer.write();
            while self.cursor_row > self.scroll_bottom {
                buf.scroll_region_up(self.scroll_top, self.scroll_bottom, template);
                self.cursor_row -= 1;
            }
        }
    }

    fn enter_alternate_screen(&mut self) {
        self.modes.alternate_screen = true;
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
        self.modes.alternate_screen = false;
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

    fn full_reset(&mut self) {
        self.reset_state();
    }

    // ────────────────────────────────────────────────────────────────
    // Public API
    // ────────────────────────────────────────────────────────────────

    /// Whether the emulator is currently showing the alternate screen.
    pub fn is_alternate_screen(&self) -> bool {
        self.modes.alternate_screen
    }

    /// Force-exit the alternate screen if active.
    /// Used when a command exits without properly restoring the main screen
    /// (e.g., vim killed without :q, htop killed without q).
    /// Returns true if we were on the alternate screen and recovered.
    pub fn recover_from_alternate_screen(&mut self) -> bool {
        if self.modes.alternate_screen {
            self.exit_alternate_screen();
            tracing::info!("Auto-recovered from stale alternate screen");
            true
        } else {
            false
        }
    }

    pub fn buffer(&self) -> Buffer {
        self.buffer.read().clone()
    }

    /// Snapshot the currently active buffer (main or alternate).
    pub fn snapshot(&self) -> Buffer {
        self.buffer.read().clone()
    }

    /// Return the current buffer's generation counter.
    pub fn buffer_generation(&self) -> u64 {
        self.buffer.read().generation()
    }

    /// Return the number of scrollback lines without cloning the buffer.
    pub fn scrollback_len(&self) -> usize {
        self.buffer.read().scrollback.len()
    }

    /// Snapshot the main buffer (returns the main buffer even if the
    /// alternate screen is currently active).
    pub fn snapshot_main(&self) -> Buffer {
        if self.modes.alternate_screen {
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
        if self.modes.alternate_screen {
            self.buffer.read().clone()
        } else {
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
    pub fn is_cursor_visible(&self) -> bool {
        self.modes.cursor_visible
    }

    pub fn dimensions(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }

    /// Whether SGR extended mouse coordinates (?1006) is enabled.
    pub fn mouse_sgr_enabled(&self) -> bool {
        self.modes.mouse_sgr
    }

    /// Whether any mouse tracking mode is enabled (1002 or 1003).
    pub fn mouse_tracking_enabled(&self) -> bool {
        self.modes.mouse_button_tracking || self.modes.mouse_any_tracking
    }

    /// Current window title (set via OSC 0 / OSC 2).
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Whether bracketed paste mode is enabled (?2004).
    pub fn bracketed_paste_enabled(&self) -> bool {
        self.modes.bracketed_paste
    }

    /// Whether focus reporting is enabled (?1004).
    pub fn focus_reporting_enabled(&self) -> bool {
        self.modes.focus_reporting
    }

    /// Check and clear the bell pending flag.
    pub fn drain_bell(&mut self) -> bool {
        std::mem::replace(&mut self.bell_pending, false)
    }

    /// Current cursor style set by DECSCUSR.
    pub fn cursor_style(&self) -> CursorStyle {
        self.cursor_style
    }

    /// Most recent DCS data (e.g. kitty graphics protocol payload).
    pub fn dcs_buffer(&self) -> &str {
        &self.dcs_buffer_str
    }

    /// Get the list of stored inline Sixel images.
    pub fn sixel_images(&self) -> &[(usize, usize, String)] {
        &self.sixel_images
    }

    /// Clear all stored inline Sixel images.
    pub fn clear_sixel_images(&mut self) {
        self.sixel_images.clear();
    }

    /// Drain pending response bytes (e.g. DA1 replies) that should be
    /// written back to the child PTY.
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

// ────────────────────────────────────────────────────────────────
// vte::Perform implementation
// ────────────────────────────────────────────────────────────────

impl Perform for VttyEmulator {
    fn print(&mut self, c: char) {
        self.write_char(c);
    }

    fn execute(&mut self, byte: u8) {
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
            0x0a => {
                // LF: move cursor down AND reset to column 0 (Unix newline).
                // vte routes ALL 0x0A bytes to execute(), so we do CR+LF here
                // to maintain the same behavior the old parser had when \n
                // appeared inside text (which went to write_char, not process_control).
                self.wrap_pending = false;
                self.cursor_col = 0;
                self.cursor_row += 1;
                self.check_scroll();
            }
            0x0b | 0x0c => {
                // VT/FF: move cursor down only (no carriage return).
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

    fn csi_dispatch(
        &mut self,
        params: &vte::Params,
        intermediates: &[u8],
        ignore: bool,
        action: char,
    ) {
        self.process_csi(params, intermediates, ignore, action);
    }

    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        self.process_osc(params);
    }

    fn esc_dispatch(&mut self, _intermediates: &[u8], _ignore: bool, byte: u8) {
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
                    let template = Some(&self.make_blank());
                    let mut buf = self.buffer.write();
                    buf.scroll_region_down(self.scroll_top, self.scroll_bottom, template);
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

    fn hook(
        &mut self,
        _params: &vte::Params,
        intermediates: &[u8],
        _ignore: bool,
        action: char,
    ) {
        self.dcs_raw.clear();
        self.dcs_intermediates = intermediates.to_vec();
        self.dcs_final = action;
    }

    fn put(&mut self, byte: u8) {
        self.dcs_raw.push(byte);
    }

    fn unhook(&mut self) {
        if self.dcs_final == 'q' {
            let empty_params = vte::Params::default();
            let intermediates = self.dcs_intermediates.clone();
            let final_byte = self.dcs_final;
            let data = self.dcs_raw.clone();
            self.process_dcs(
                &empty_params,
                &intermediates,
                final_byte,
                &data,
            );
        }
        // Other DCS sequences are silently consumed.
        self.dcs_raw.clear();
        self.dcs_intermediates.clear();
        self.dcs_final = '\0';
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Cursor movement boundary clamping ──

    #[test]
    fn test_cursor_movement() {
        let mut emu = VttyEmulator::new(10, 10, 100);
        emu.feed_str("Hello\x1b[2;3H");
        assert_eq!(emu.cursor(), (1, 2));
    }

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

    // ── Scrollback ──

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
    fn test_multi_line_scroll_scrollback_capacity() {
        let mut emu = VttyEmulator::new(3, 5, 5);
        for i in 0..20 {
            emu.feed_str(&format!("L{}\r\n", i));
        }
        // scrollback capped at 5
        assert_eq!(emu.scrollback_len(), 5);
    }

    // ── Save/restore cursor ──

    #[test]
    fn test_save_restore_cursor() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("\x1b[2;5H"); // move to row 2, col 5
        emu.feed_str("\x1b7"); // save
        emu.feed_str("\x1b[1;1H"); // move to home
        assert_eq!(emu.cursor(), (0, 0));
        emu.feed_str("\x1b8"); // restore
        assert_eq!(emu.cursor(), (1, 4));
    }

    // ── SGR / color ──

    #[test]
    fn test_sgr_foreground_color() {
        let mut emu = VttyEmulator::new(5, 20, 100);
        emu.feed_str("\x1b[38;2;255;0;0mX"); // bright red
        let buf = emu.buffer();
        assert_eq!(buf.rows[0][0].ch, 'X');
        assert_eq!(buf.rows[0][0].fg, [255, 0, 0]);
    }

    #[test]
    fn test_sgr_256_color() {
        let mut emu = VttyEmulator::new(5, 20, 100);
        emu.feed_str("\x1b[38;5;196mY"); // color 196 from palette
        let buf = emu.buffer();
        assert_eq!(buf.rows[0][0].ch, 'Y');
        // Color 196 = [255, 0, 0] from the 6x6x6 cube (r=5, g=0, b=0)
        assert_eq!(buf.rows[0][0].fg, [255, 0, 0]);
    }

    #[test]
    fn test_sgr_reset() {
        let mut emu = VttyEmulator::new(5, 20, 100);
        emu.feed_str("\x1b[1;3;38;2;0;255;0mX\x1b[0mY");
        let buf = emu.buffer();
        // X should have bold + italic + green
        assert!(buf.rows[0][0].bold);
        assert!(buf.rows[0][0].italic);
        assert_eq!(buf.rows[0][0].fg, [0, 255, 0]);
        // Y should be reset to defaults
        assert!(!buf.rows[0][1].bold);
        assert!(!buf.rows[0][1].italic);
        assert_eq!(buf.rows[0][1].fg, [204, 204, 204]);
    }

    // ── Auto-wrap ──

    #[test]
    fn test_auto_wrap() {
        let mut emu = VttyEmulator::new(5, 5, 100);
        emu.feed_str("abcde"); // fills first row exactly
        assert_eq!(emu.cursor(), (0, 4));
        assert!(emu.wrap_pending);
        emu.feed_str("f"); // should wrap to next line
        assert_eq!(emu.cursor(), (1, 1));
        assert!(!emu.wrap_pending);
        let buf = emu.buffer();
        assert_eq!(buf.rows[0][4].ch, 'e');
        assert_eq!(buf.rows[1][0].ch, 'f');
    }

    #[test]
    fn test_auto_wrap_disabled() {
        let mut emu = VttyEmulator::new(5, 5, 100);
        emu.feed_str("\x1b[?7l"); // disable auto-wrap
        emu.feed_str("abcdef");
        // With wrap disabled, cursor stays at last column, 'f' overwrites 'e'
        assert_eq!(emu.cursor_col, 4);
        let buf = emu.buffer();
        assert_eq!(buf.rows[0][4].ch, 'f');
        assert_eq!(buf.rows[1][0].ch, ' ');
    }

    // ── Erase ──

    #[test]
    fn test_erase_in_line() {
        let mut emu = VttyEmulator::new(1, 10, 100);
        emu.feed_str("ABCDEFGHIJ\x1b[1;5H\x1b[K");
        let buf = emu.buffer();
        assert_eq!(buf.rows[0][0].ch, 'A');
        assert_eq!(buf.rows[0][3].ch, 'D');
        assert_eq!(buf.rows[0][4].ch, ' ');
        assert_eq!(buf.rows[0][9].ch, ' ');
    }

    #[test]
    fn test_erase_entire_screen() {
        let mut emu = VttyEmulator::new(3, 5, 100);
        emu.feed_str("AAAAABBBBBCCCCC\x1b[2J");
        let buf = emu.buffer();
        for row in &buf.rows {
            for cell in row {
                assert_eq!(cell.ch, ' ');
            }
        }
    }

    // ── Scrolling region ──

    #[test]
    fn test_scroll_region() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        for i in 0..5 {
            let c = char::from_digit(i as u32, 10).unwrap();
            emu.feed_str(&format!("{}\n", c));
        }
        // After writing 5 lines with \n, the 5th \n causes a scroll.
        // Visible rows are now "1","2","3","4","" (0 was pushed to scrollback).
        // Set scroll region to rows 2-4 (1-based, i.e. 0-based rows 1-3)
        emu.feed_str("\x1b[2;4r"); // DECSTBM
        emu.feed_str("\x1b[S"); // SU (scroll up)
        let buf = emu.buffer();
        // Row 0 = "1" — outside region, unchanged
        assert_eq!(buf.rows[0][0].ch, '1');
        // Row 1 is "3" (shifted up from "2" which was pushed out), row 2 is "4"
        assert_eq!(buf.rows[1][0].ch, '3');
        // Row 3 is blank (new line scrolled in from bottom of region)
        assert_eq!(buf.rows[3][0].ch, ' ');
        // Row 4 = "" — outside region, unchanged
        assert_eq!(buf.rows[4][0].ch, ' ');
    }

    // ── Line insertion / deletion ──

    #[test]
    fn test_insert_line() {
        let mut emu = VttyEmulator::new(3, 10, 100);
        emu.feed_str("AAA\nBBB\nCCC");
        emu.feed_str("\x1b[2;1H"); // row 2 (0-based: 1)
        emu.feed_str("\x1b[L"); // insert line
        let buf = emu.buffer();
        assert_eq!(buf.rows[0][0].ch, 'A');
        assert_eq!(buf.rows[1][0].ch, ' '); // inserted blank
        assert_eq!(buf.rows[2][0].ch, 'B'); // shifted down, CCC lost
    }

    #[test]
    fn test_delete_line() {
        let mut emu = VttyEmulator::new(3, 10, 100);
        emu.feed_str("AAA\nBBB\nCCC");
        emu.feed_str("\x1b[2;1H"); // row 2 (0-based: 1)
        emu.feed_str("\x1b[M"); // delete line
        let buf = emu.buffer();
        assert_eq!(buf.rows[0][0].ch, 'A');
        assert_eq!(buf.rows[1][0].ch, 'C'); // shifted up
        assert_eq!(buf.rows[2][0].ch, ' '); // new blank
    }

    // ── Wide characters ──

    #[test]
    fn test_wide_char() {
        let mut emu = VttyEmulator::new(1, 10, 100);
        emu.feed_str("AB你CD");
        let buf = emu.buffer();
        assert_eq!(buf.rows[0][0].ch, 'A');
        assert_eq!(buf.rows[0][1].ch, 'B');
        assert_eq!(buf.rows[0][2].ch, '你');
        assert_eq!(buf.rows[0][2].width, 2);
        assert_eq!(buf.rows[0][3].width, 0); // continuation
        assert_eq!(buf.rows[0][4].ch, 'C');
        assert_eq!(buf.rows[0][5].ch, 'D');
    }

    #[test]
    fn test_wide_char_at_right_margin() {
        let mut emu = VttyEmulator::new(1, 5, 100);
        emu.feed_str("ABCD你");
        let buf = emu.buffer();
        // "ABCD" fills cols 0-3, cursor at col 4.
        // '你' is width 2, cursor_col + 1 = 5 >= cols (5), so it wraps.
        // With only 1 row, the wrap triggers a scroll: "ABCD " goes to scrollback.
        // Then 你 is written at (0,0) with continuation at (0,1).
        assert_eq!(buf.rows[0][0].ch, '你');
        assert_eq!(buf.rows[0][0].width, 2);
        assert_eq!(buf.rows[0][1].width, 0); // continuation
        assert_eq!(buf.rows[0][4].ch, ' '); // was D, now blank after scroll
        assert_eq!(buf.scrollback.len(), 1);
        assert_eq!(buf.scrollback[0][3].ch, 'D'); // D is in scrollback
    }

    // ── Tab ──

    #[test]
    fn test_tab() {
        let mut emu = VttyEmulator::new(1, 20, 100);
        emu.feed_str("A\tB");
        assert_eq!(emu.cursor_col, 9); // tab from col 1 → col 8
        let buf = emu.buffer();
        assert_eq!(buf.rows[0][0].ch, 'A');
        assert_eq!(buf.rows[0][8].ch, 'B');
    }

    // ── Backspace ──

    #[test]
    fn test_backspace() {
        let mut emu = VttyEmulator::new(1, 20, 100);
        emu.feed_str("ABC\x08");
        assert_eq!(emu.cursor_col, 2);
    }

    // ── OSC ──

    #[test]
    fn test_osc_title() {
        let mut emu = VttyEmulator::new(5, 20, 100);
        emu.feed_str("\x1b]2;hello world\x07");
        assert_eq!(emu.title(), "hello world");
    }

    // ── DCS ──

    #[test]
    fn test_dcs_sixel() {
        let mut emu = VttyEmulator::new(5, 20, 100);
        emu.feed_str("\x1bP?q#0;1;1;1AB\x1b\\");
        assert!(emu.sixel_images().len() >= 1, "should have stored a sixel image");
    }

    // ── Full reset ──

    #[test]
    fn test_full_reset() {
        let mut emu = VttyEmulator::new(5, 20, 100);
        emu.feed_str("\x1b[1;38;2;255;0;0mHello\x1b[3;5H");
        emu.feed_str("\x1bc"); // full reset via ESC c
        assert_eq!(emu.cursor(), (0, 0));
        assert!(!emu.attrs.bold);
        assert_eq!(emu.attrs.fg, [204, 204, 204]);
    }

    // ── Insert mode ──

    #[test]
    fn test_insert_mode() {
        let mut emu = VttyEmulator::new(1, 10, 100);
        emu.feed_str("ABCDE");
        emu.feed_str("\x1b[1;3H"); // col 3 (0-based: 2)
        emu.feed_str("\x1b[4h"); // set insert mode (DECIM)
        emu.feed_str("XY");
        let buf = emu.buffer();
        // "AB" + "XY" + "CDE" → "ABXYCDE" truncated to 10
        assert_eq!(buf.rows[0][0].ch, 'A');
        assert_eq!(buf.rows[0][1].ch, 'B');
        assert_eq!(buf.rows[0][2].ch, 'X');
        assert_eq!(buf.rows[0][3].ch, 'Y');
        assert_eq!(buf.rows[0][4].ch, 'C');
        assert_eq!(buf.rows[0][5].ch, 'D');
        assert_eq!(buf.rows[0][6].ch, 'E');
    }

    // ── Alternate screen ──

    #[test]
    fn test_alternate_screen() {
        let mut emu = VttyEmulator::new(3, 10, 100);
        emu.feed_str("main");
        emu.feed_str("\x1b[?1049h"); // enter alt screen
        assert!(emu.is_alternate_screen());
        emu.feed_str("alt");
        let buf = emu.snapshot();
        // Should see "alt" on the alt screen, not "main"
        assert_eq!(buf.rows[0][0].ch, 'a');

        emu.feed_str("\x1b[?1049l"); // exit alt screen
        assert!(!emu.is_alternate_screen());
        let buf = emu.snapshot();
        // Should see "main" restored
        assert_eq!(buf.rows[0][0].ch, 'm');
    }

    // ── DA1 response ──

    #[test]
    fn test_da1_response() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("\x1b[c"); // DA1
        let resp = emu.drain_responses();
        assert_eq!(resp, b"\x1b[?1;0c");
    }

    // ── DSR (cursor position report) ──

    #[test]
    fn test_dsr_cpr() {
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("\x1b[3;7H"); // move to row 3, col 7
        emu.feed_str("\x1b[6n"); // DSR - request CPR
        let resp = emu.drain_responses();
        assert_eq!(resp, b"\x1b[3;7R");
    }

    // ── Reverse video ──

    #[test]
    fn test_reverse_video() {
        let mut emu = VttyEmulator::new(1, 10, 100);
        emu.feed_str("\x1b[7mX");
        let buf = emu.buffer();
        // With reverse: fg and bg are swapped
        assert_eq!(buf.rows[0][0].fg, [0, 0, 0]);
        assert_eq!(buf.rows[0][0].bg, [204, 204, 204]);
    }

    // ── Scrolling with colored background ──

    #[test]
    fn test_scroll_preserves_bg() {
        let mut emu = VttyEmulator::new(1, 10, 100);
        emu.feed_str("\x1b[48;2;0;0;255m"); // blue bg
        emu.feed_str("X\n"); // write X and scroll
        emu.feed_str("Y");
        let buf = emu.buffer();
        // The blank line from scrolling should have blue bg
        // (since scroll uses the current attrs as template)
        // Actually after the scroll, the new blank line has the current bg.
        // The scroll happens when we write '\n', which goes to execute(0x0A).
        // At that point, the template is make_blank() which has reverse applied.
        // Since reverse is off, bg=[0,0,255].
        assert_eq!(buf.rows[0][0].bg, [0, 0, 255], "scrolled blank should preserve blue bg");
    }
}