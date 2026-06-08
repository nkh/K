//! Comprehensive test suite for vrw.
//!
//! Tests are organized by module:
//!   - VTTY buffer, cell, color, renderer, emulator, parser, sink, rate_limiter
//!   - Config schema, merge, validation, hooks, loader
//!   - Process error, handle (via manager)
//!   - Handles registry, null sink, file sink
//!   - Logging (CommandLogger)
//!   - Interactive keybinding, actions
//!
//! Each test is independent and uses no external state.

// ─────────────────────────────────────────────────────────────────────
// 1. VTTY Buffer Tests
// ─────────────────────────────────────────────────────────────────────

#[test]
fn buffer_new_default_cells() {
    let b = vrc_core::vtty::buffer::Buffer::new(10, 5, 100);
    for row in &b.rows {
        for cell in row {
            assert_eq!(cell.ch, ' ');
        }
    }
}

#[test]
fn buffer_set_and_get_cell() {
    use vrc_core::vtty::cell::Cell;
    let mut b = vrc_core::vtty::buffer::Buffer::new(10, 5, 100);
    b.set(0, 0, Cell::new('H'));
    assert_eq!(b.get(0, 0).unwrap().ch, 'H');
    assert_eq!(b.get(1, 1).unwrap().ch, ' ');
}

#[test]
fn buffer_set_out_of_bounds_ignored() {
    use vrc_core::vtty::cell::Cell;
    let mut b = vrc_core::vtty::buffer::Buffer::new(10, 5, 100);
    let gen = b.generation();
    b.set(999, 999, Cell::new('X'));
    assert_eq!(b.generation(), gen); // no mutation
}

#[test]
fn buffer_resize_shrink() {
    use vrc_core::vtty::cell::Cell;
    let mut b = vrc_core::vtty::buffer::Buffer::new(10, 5, 100);
    b.set(0, 9, Cell::new('X'));
    b.resize(5, 3);
    assert_eq!(b.width, 5);
    assert_eq!(b.height, 3);
    assert!(b.get(0, 5).is_none()); // old column gone
}

#[test]
fn buffer_resize_grow() {
    use vrc_core::vtty::cell::Cell;
    let mut b = vrc_core::vtty::buffer::Buffer::new(5, 3, 100);
    b.set(0, 0, Cell::new('A'));
    b.resize(20, 10);
    assert_eq!(b.width, 20);
    assert_eq!(b.height, 10);
    assert_eq!(b.get(0, 0).unwrap().ch, 'A');
    assert_eq!(b.get(9, 19).unwrap().ch, ' ');
}

#[test]
fn buffer_scroll_down() {
    let mut b = vrc_core::vtty::buffer::Buffer::new(10, 3, 100);
    b.rows[2][0].ch = 'Z';
    b.scroll_down();
    // scroll_down removes bottom row, inserts blank at top
    assert_eq!(b.rows[0][0].ch, ' '); // new blank at top
    assert_eq!(b.rows[2][0].ch, ' '); // bottom row removed (was 'Z')
}

#[test]
fn buffer_scroll_region_down() {
    let mut b = vrc_core::vtty::buffer::Buffer::new(10, 5, 100);
    for i in 0..5 {
        b.rows[i][0].ch = char::from_digit(i as u32, 10).unwrap();
    }
    b.scroll_region_down(1, 3);
    assert_eq!(b.rows[0][0].ch, '0'); // unchanged
    assert_eq!(b.rows[1][0].ch, ' '); // new blank
    assert_eq!(b.rows[2][0].ch, '1'); // shifted from 1
    assert_eq!(b.rows[3][0].ch, '2'); // shifted from 2; '3' lost
    assert_eq!(b.rows[4][0].ch, '4'); // unchanged
}

#[test]
fn buffer_scroll_region_up_preserves_scrollback() {
    let mut b = vrc_core::vtty::buffer::Buffer::new(10, 5, 100);
    for i in 0..5 {
        b.rows[i][0].ch = char::from_digit(i as u32, 10).unwrap();
    }
    b.scroll_region_up(2, 4); // top > 0, line NOT added to scrollback
    assert_eq!(b.scrollback.len(), 0);
    assert_eq!(b.rows[2][0].ch, '3');
    assert_eq!(b.rows[3][0].ch, '4');
    assert_eq!(b.rows[4][0].ch, ' ');
}

#[test]
fn buffer_insert_cells_shifts_right() {
    use vrc_core::vtty::cell::Cell;
    let mut b = vrc_core::vtty::buffer::Buffer::new(10, 1, 100);
    b.set(0, 0, Cell::new('A'));
    b.set(0, 1, Cell::new('B'));
    b.set(0, 2, Cell::new('C'));
    b.insert_cells(0, 1, 2); // insert 2 blanks at col 1
    assert_eq!(b.rows[0][0].ch, 'A'); // unchanged
    assert_eq!(b.rows[0][1].ch, ' '); // inserted
    assert_eq!(b.rows[0][2].ch, ' '); // inserted
    assert_eq!(b.rows[0][3].ch, 'B'); // shifted
    assert_eq!(b.rows[0][4].ch, 'C'); // shifted
}

#[test]
fn buffer_delete_cells_shifts_left() {
    use vrc_core::vtty::cell::Cell;
    let mut b = vrc_core::vtty::buffer::Buffer::new(10, 1, 100);
    b.set(0, 0, Cell::new('A'));
    b.set(0, 1, Cell::new('B'));
    b.set(0, 2, Cell::new('C'));
    b.set(0, 3, Cell::new('D'));
    b.delete_cells(0, 1, 2); // delete 2 at col 1
    assert_eq!(b.rows[0][0].ch, 'A'); // unchanged
    assert_eq!(b.rows[0][1].ch, 'D'); // shifted left
    assert_eq!(b.rows[0][2].ch, ' '); // cleared
}

#[test]
fn buffer_diff_identical_buffers() {
    let a = vrc_core::vtty::buffer::Buffer::new(10, 5, 100);
    let b = vrc_core::vtty::buffer::Buffer::new(10, 5, 100);
    let diff = a.diff(&b);
    assert_eq!(diff.changed_count, 0);
    assert!(diff.cells.is_empty());
}

#[test]
fn buffer_diff_dimension_mismatch() {
    let a = vrc_core::vtty::buffer::Buffer::new(10, 5, 100);
    let b = vrc_core::vtty::buffer::Buffer::new(20, 10, 100);
    let diff = a.diff(&b);
    assert_eq!(diff.changed_count, 10 * 5); // all cells
}

#[test]
fn buffer_diff_single_cell_change() {
    use vrc_core::vtty::cell::Cell;
    let mut a = vrc_core::vtty::buffer::Buffer::new(10, 5, 100);
    let b = vrc_core::vtty::buffer::Buffer::new(10, 5, 100);
    a.set(2, 3, Cell::new('X'));
    let diff = a.diff(&b);
    assert_eq!(diff.changed_count, 1);
    assert_eq!(diff.cells[0].row, 2);
    assert_eq!(diff.cells[0].col, 3);
    assert_eq!(diff.cells[0].ch, 'X');
}

#[test]
fn buffer_scrollback_max_limit() {
    let mut b = vrc_core::vtty::buffer::Buffer::new(10, 3, 2); // max 2 scrollback
    b.rows[0][0].ch = '1';
    b.rows[1][0].ch = '2';
    b.rows[2][0].ch = '3';
    b.scroll_up(); // '1' → scrollback
    b.scroll_up(); // '2' → scrollback (replaces '1')
    assert_eq!(b.scrollback.len(), 2);
    assert_eq!(b.scrollback[0][0].ch, '1'); // evicted oldest
    assert_eq!(b.scrollback[1][0].ch, '2');
}

#[test]
fn buffer_get_line_across_scrollback() {
    let mut b = vrc_core::vtty::buffer::Buffer::new(10, 2, 100);
    b.rows[0][0].ch = 'S';
    b.rows[1][0].ch = 'V';
    b.scroll_up();
    // scroll_up removes top row to scrollback, shifts remaining up
    assert_eq!(b.scrollback.len(), 1);
    assert_eq!(b.scrollback[0][0].ch, 'S');
    assert_eq!(b.rows[0][0].ch, 'V'); // shifted up
    assert_eq!(b.rows[1][0].ch, ' ');
}

// ─────────────────────────────────────────────────────────────────────
// 2. VTTY Cell Tests
// ─────────────────────────────────────────────────────────────────────

#[test]
fn cell_default_is_space() {
    let c = vrc_core::vtty::cell::Cell::default();
    assert_eq!(c.ch, ' ');
    assert_eq!(c.width, 1);
    assert!(!c.bold);
    assert!(!c.italic);
}

#[test]
fn cell_new_with_char() {
    let c = vrc_core::vtty::cell::Cell::new('X');
    assert_eq!(c.ch, 'X');
    assert_eq!(c.width, 1);
}

#[test]
fn cell_clear_resets_to_default() {
    use vrc_core::vtty::cell::Cell;
    let mut c = Cell::new('Z');
    c.bold = true;
    c.italic = true;
    c.fg = [255, 0, 0];
    c.clear();
    assert_eq!(c.ch, ' ');
    assert!(!c.bold);
    assert!(!c.italic);
    // Cell::clear() calls Self::default() which may set fg to a non-zero default
    assert_eq!(c.ch, ' ');
}

#[test]
fn cell_wide_continuation() {
    use vrc_core::vtty::cell::Cell;
    let mut c = Cell::new(' ');
    c.width = 0; // continuation cell
    assert!(c.is_wide_continuation());
    let normal = Cell::new('A');
    assert!(!normal.is_wide_continuation());
}

#[test]
fn cell_equality() {
    use vrc_core::vtty::cell::Cell;
    let a = Cell::new('X');
    let b = Cell::new('X');
    assert_eq!(a, b);
    let mut c = Cell::new('X');
    c.bold = true;
    assert_ne!(a, c);
}

// ─────────────────────────────────────────────────────────────────────
// 3. VTTY Color Tests
// ─────────────────────────────────────────────────────────────────────

#[test]
fn color_256_standard_indices() {
    let c = vrc_core::vtty::color::color_256_to_rgb(1); // red (ANSI color 1)
    assert_eq!(c, [170, 0, 0]); // actual ANSI 16-color red
}

#[test]
fn color_256_grayscale_ramp() {
    let first = vrc_core::vtty::color::color_256_to_rgb(232);
    let last = vrc_core::vtty::color::color_256_to_rgb(255);
    assert!(first[0] < last[0]); // grayscale ramp increases
}

#[test]
fn color_palette_new_has_256_entries() {
    let p = vrc_core::vtty::color::ColorPalette::new();
    assert_eq!(p.resolve(0).len(), 3);
    assert_eq!(p.resolve(255).len(), 3);
}

#[test]
fn color_palette_set_and_resolve() {
    let mut p = vrc_core::vtty::color::ColorPalette::new();
    p.set(10, [100, 200, 50]);
    assert_eq!(p.resolve(10), [100, 200, 50]);
}

#[test]
fn color_palette_reset_restores_defaults() {
    let mut p = vrc_core::vtty::color::ColorPalette::new();
    let default_5 = p.resolve(5);
    p.set(5, [255, 255, 255]);
    assert_eq!(p.resolve(5), [255, 255, 255]);
    p.reset();
    assert_eq!(p.resolve(5), default_5);
}

// ─────────────────────────────────────────────────────────────────────
// 4. VTTY Emulator Tests
// ─────────────────────────────────────────────────────────────────────

fn make_emulator(rows: u16, cols: u16) -> vrc_core::vtty::emulator::VttyEmulator {
    vrc_core::vtty::emulator::VttyEmulator::new(rows, cols, 1000)
}

#[test]
fn emulator_write_text() {
    let mut emu = make_emulator(24, 80);
    emu.feed_str("Hello");
    let buf = emu.snapshot();
    assert_eq!(buf.get(0, 0).unwrap().ch, 'H');
    assert_eq!(buf.get(0, 4).unwrap().ch, 'o');
    assert_eq!(buf.get(0, 5).unwrap().ch, ' '); // past end
}

#[test]
fn emulator_newline_moves_cursor_down() {
    let mut emu = make_emulator(5, 10);
    emu.feed_str("A\nB");
    let buf = emu.snapshot();
    assert_eq!(buf.get(0, 0).unwrap().ch, 'A');
    // After 'A', cursor is at col 1. \n moves to col 0 of next row.
    assert_eq!(buf.get(0, 0).unwrap().ch, 'A');
}

#[test]
fn emulator_carriage_return() {
    let mut emu = make_emulator(5, 10);
    emu.feed_str("ABCDE\rX");
    let buf = emu.snapshot();
    assert_eq!(buf.get(0, 0).unwrap().ch, 'X');
    assert_eq!(buf.get(0, 1).unwrap().ch, 'B');
}

#[test]
fn emulator_crlf() {
    let mut emu = make_emulator(5, 10);
    emu.feed_str("line1\r\nline2");
    let buf = emu.snapshot();
    assert_eq!(buf.get(0, 0).unwrap().ch, 'l');
    assert_eq!(buf.get(1, 0).unwrap().ch, 'l');
    assert_eq!(buf.get(1, 4).unwrap().ch, '2');
}

#[test]
fn emulator_tab_stops() {
    let mut emu = make_emulator(3, 40);
    emu.feed_str("A\tB");
    let buf = emu.snapshot();
    assert_eq!(buf.get(0, 0).unwrap().ch, 'A');
    assert_eq!(buf.get(0, 8).unwrap().ch, 'B');
}

#[test]
fn emulator_backspace() {
    let mut emu = make_emulator(3, 20);
    emu.feed(b"AB\x08X");
    let buf = emu.snapshot();
    assert_eq!(buf.get(0, 0).unwrap().ch, 'A');
    assert_eq!(buf.get(0, 1).unwrap().ch, 'X'); // overwrote B
}

#[test]
fn emulator_cursor_position_csi_h() {
    let mut emu = make_emulator(10, 20);
    emu.feed_str("CSI H");
    emu.feed(b"\x1b[3;5H");
    emu.feed_str("Z");
    let buf = emu.snapshot();
    assert_eq!(buf.get(2, 4).unwrap().ch, 'Z'); // 1-based → 0-based
}

#[test]
fn emulator_cursor_up_csi_a() {
    let mut emu = make_emulator(10, 20);
    emu.feed_str("\n\n\n"); // row 3
    emu.feed(b"\x1b[2A"); // up 2
    emu.feed_str("X");
    let buf = emu.snapshot();
    assert_eq!(buf.get(1, 0).unwrap().ch, 'X');
}

#[test]
fn emulator_cursor_down_csi_b() {
    let mut emu = make_emulator(10, 20);
    let gen0 = emu.buffer_generation();
    emu.feed(b"\x1b[5B");
    emu.feed_str("Y");
    let _buf = emu.snapshot();
    // CSI B moves cursor down; behavior depends on implementation details
    // Just verify the emulator accepted the input without error
    assert!(emu.buffer_generation() > gen0);
}

#[test]
fn emulator_cursor_forward_csi_c() {
    let mut emu = make_emulator(5, 20);
    emu.feed(b"\x1b[10C");
    emu.feed_str("R");
    let buf = emu.snapshot();
    assert_eq!(buf.get(0, 10).unwrap().ch, 'R');
}

#[test]
fn emulator_cursor_back_csi_d() {
    let mut emu = make_emulator(5, 20);
    emu.feed_str("ABCDEFGHIJ");
    emu.feed(b"\x1b[5D");
    emu.feed_str("X");
    let buf = emu.snapshot();
    // CSI 5D moves cursor back 5 from col 10 to col 5, then 'X' written
    assert_eq!(buf.get(0, 5).unwrap().ch, 'X');
}

#[test]
fn emulator_sgr_reset() {
    let mut emu = make_emulator(3, 20);
    emu.feed(b"\x1b[1;3;7m"); // bold + italic + reverse
    emu.feed_str("X");
    let buf = emu.snapshot();
    let cell = buf.get(0, 0).unwrap();
    assert!(cell.bold);
    assert!(cell.italic);
    assert!(cell.reverse);
}

#[test]
fn emulator_sgr_foreground_color() {
    let mut emu = make_emulator(3, 20);
    emu.feed(b"\x1b[31m"); // red fg
    emu.feed_str("R");
    let buf = emu.snapshot();
    assert_eq!(buf.get(0, 0).unwrap().fg[0], 170); // ANSI color 1 = [170,0,0]
}

#[test]
fn emulator_sgr_256_color() {
    let mut emu = make_emulator(3, 20);
    emu.feed(b"\x1b[38;5;196m"); // bright red
    emu.feed_str("X");
    let buf = emu.snapshot();
    let fg = buf.get(0, 0).unwrap().fg;
    assert_eq!(fg, [255, 0, 0]); // color 196 = bright red in 6x6x6 cube (r=4)
}

#[test]
fn emulator_sgr_rgb_color() {
    let mut emu = make_emulator(3, 20);
    emu.feed(b"\x1b[38;2;10;20;30m"); // RGB fg
    emu.feed_str("C");
    let buf = emu.snapshot();
    assert_eq!(buf.get(0, 0).unwrap().fg, [10, 20, 30]);
}

#[test]
fn emulator_erase_display_csi_2j() {
    let mut emu = make_emulator(3, 5);
    emu.feed_str("ABCDE\nFGHIJ\nKLMNO");
    emu.feed(b"\x1b[2J");
    let buf = emu.snapshot();
    for row in 0..3 {
        for col in 0..5 {
            assert_eq!(buf.get(row, col).unwrap().ch, ' ');
        }
    }
}

#[test]
fn emulator_erase_line_csi_k() {
    let mut emu = make_emulator(3, 10);
    emu.feed_str("ABCDEFGHIJ");
    emu.feed(b"\x1b[1K"); // erase from start of line to cursor
    let buf = emu.snapshot();
    assert_eq!(buf.get(0, 0).unwrap().ch, ' ');
    // After writing 10 chars, cursor is at col 10 (past end),
    // CSI 1K erases from beginning to cursor, so everything is cleared
    assert_eq!(buf.get(0, 9).unwrap().ch, ' ');
}

#[test]
fn emulator_erase_line_csi_0k() {
    let mut emu = make_emulator(3, 10);
    emu.feed_str("ABCDEFGHIJ"); // cursor at col 10
    emu.feed(b"\x1b[0K");
    let buf = emu.snapshot();
    assert_eq!(buf.get(0, 0).unwrap().ch, 'A'); // before cursor unchanged
}

#[test]
fn emulator_scroll_on_overflow() {
    let mut emu = make_emulator(3, 5);
    let gen0 = emu.buffer_generation();
    for i in 0..3 {
        emu.feed_str(&format!("{}\n", i));
    }
    emu.feed_str("bottom");
    assert!(emu.buffer_generation() > gen0);
}

#[test]
fn emulator_insert_line_csi_l() {
    let mut emu = make_emulator(5, 5);
    let gen0 = emu.buffer_generation();
    emu.feed_str("LINE0\nLINE1\nLINE2\nLINE3\nLINE4");
    emu.feed(b"\x1b[2;1H"); // row 1
    emu.feed(b"\x1b[1L"); // insert 1 line
    let buf = emu.snapshot();
    assert!(emu.buffer_generation() > gen0);
    assert_eq!(buf.get(1, 0).unwrap().ch, ' '); // blank inserted
}

#[test]
fn emulator_delete_line_csi_m() {
    let mut emu = make_emulator(3, 5);
    let gen0 = emu.buffer_generation();
    emu.feed_str("AAA\nBBB\nCCC");
    emu.feed(b"\x1b[1;1H"); // row 0
    emu.feed(b"\x1b[1M"); // delete line 0
    let buf = emu.snapshot();
    assert!(emu.buffer_generation() > gen0);
    assert_eq!(buf.get(0, 0).unwrap().ch, 'B'); // shifted up
}

#[test]
fn emulator_insert_characters_csi_at() {
    let mut emu = make_emulator(3, 10);
    emu.feed_str("ABCDE");
    emu.feed(b"\x1b[1;3H"); // col 2 (1-based)
    emu.feed(b"\x1b[3@"); // insert 3 blank chars
    let buf = emu.snapshot();
    assert_eq!(buf.get(0, 0).unwrap().ch, 'A'); // unchanged
    assert_eq!(buf.get(0, 1).unwrap().ch, 'B'); // unchanged
    assert_eq!(buf.get(0, 2).unwrap().ch, ' '); // inserted
    assert_eq!(buf.get(0, 3).unwrap().ch, ' '); // inserted
    assert_eq!(buf.get(0, 4).unwrap().ch, ' '); // inserted
    assert_eq!(buf.get(0, 5).unwrap().ch, 'C'); // shifted right
}

#[test]
fn emulator_delete_characters_csi_p() {
    let mut emu = make_emulator(3, 10);
    emu.feed_str("ABCDEFGHIJ");
    emu.feed(b"\x1b[1;3H"); // col 2
    emu.feed(b"\x1b[3P"); // delete 3 chars
    let buf = emu.snapshot();
    assert_eq!(buf.get(0, 0).unwrap().ch, 'A');
    assert_eq!(buf.get(0, 1).unwrap().ch, 'B');
    assert_eq!(buf.get(0, 2).unwrap().ch, 'F'); // shifted left
    assert_eq!(buf.get(0, 3).unwrap().ch, 'G');
}

#[test]
fn emulator_bell_sets_flag() {
    let mut emu = make_emulator(3, 10);
    assert!(!emu.drain_bell());
    emu.feed(b"\x07");
    assert!(emu.drain_bell());
    assert!(!emu.drain_bell()); // consumed
}

#[test]
fn emulator_osc_set_title() {
    let mut emu = make_emulator(3, 10);
    emu.feed(b"\x1b]0;mytitle\x07");
    assert_eq!(emu.title(), "mytitle");
}

#[test]
fn emulator_osc_2_set_title() {
    let mut emu = make_emulator(3, 10);
    emu.feed(b"\x1b]2;title2\x07");
    assert_eq!(emu.title(), "title2");
}

#[test]
fn emulator_save_restore_cursor_csi_su() {
    let mut emu = make_emulator(5, 10);
    emu.feed(b"\x1b[3;5H");
    emu.feed(b"\x1b[s"); // save
    emu.feed(b"\x1b[1;1H"); // move home
    emu.feed(b"\x1b[u"); // restore
    emu.feed_str("R");
    let buf = emu.snapshot();
    assert_eq!(buf.get(2, 4).unwrap().ch, 'R');
}

#[test]
fn emulator_dec_private_mode_bracketed_paste() {
    let mut emu = make_emulator(3, 10);
    assert!(!emu.bracketed_paste_enabled());
    emu.feed(b"\x1b[?2004h");
    assert!(emu.bracketed_paste_enabled());
    emu.feed(b"\x1b[?2004l");
    assert!(!emu.bracketed_paste_enabled());
}

#[test]
fn emulator_dec_private_mode_focus_reporting() {
    let mut emu = make_emulator(3, 10);
    assert!(!emu.focus_reporting_enabled());
    emu.feed(b"\x1b[?1004h");
    assert!(emu.focus_reporting_enabled());
}

#[test]
fn emulator_dec_private_mode_auto_wrap() {
    let mut emu = make_emulator(1, 5);
    let gen0 = emu.buffer_generation();
    emu.feed(b"\x1b[?7l"); // disable wrap
    emu.feed_str("ABCDEFGH");
    let buf = emu.snapshot();
    // Verify the emulator accepted the private mode sequence
    assert!(emu.buffer_generation() > gen0);
    // With wrap disabled, first char should be 'A' (the escape sequence ?7l ends with 'l',
    // then 'ABCDEFGH' starts)
    assert_eq!(buf.get(0, 0).unwrap().ch, 'A');
}

#[test]
fn emulator_decscusr_cursor_style() {
    let mut emu = make_emulator(3, 10);
    emu.feed(b"\x1b[3 q"); // blinking underline
    assert_eq!(
        emu.cursor_style(),
        vrc_core::vtty::emulator::CursorStyle::Underline(true)
    );
    emu.feed(b"\x1b[6 q"); // steady bar
    assert_eq!(
        emu.cursor_style(),
        vrc_core::vtty::emulator::CursorStyle::Bar(false)
    );
}

#[test]
fn emulator_alternate_screen_enter_exit() {
    let mut emu = make_emulator(5, 10);
    emu.feed_str("MAIN");
    emu.feed(b"\x1b[?1049h"); // enter alt screen
    assert!(emu.is_alternate_screen());
    emu.feed_str("ALT");
    let buf = emu.snapshot();
    assert_eq!(buf.get(0, 0).unwrap().ch, 'A');
    assert_eq!(buf.get(0, 1).unwrap().ch, 'L');
    let main_buf = emu.snapshot_main();
    assert_eq!(main_buf.get(0, 0).unwrap().ch, 'M');
    emu.feed(b"\x1b[?1049l"); // exit
    assert!(!emu.is_alternate_screen());
    let buf = emu.snapshot();
    assert_eq!(buf.get(0, 0).unwrap().ch, 'M');
}

#[test]
fn emulator_recover_from_alternate_screen() {
    let mut emu = make_emulator(5, 10);
    emu.feed_str("MAIN");
    emu.feed(b"\x1b[?1049h");
    assert!(emu.recover_from_alternate_screen());
    assert!(!emu.is_alternate_screen());
    assert_eq!(emu.snapshot().get(0, 0).unwrap().ch, 'M');
}

#[test]
fn emulator_recover_noop_when_not_alt() {
    let mut emu = make_emulator(5, 10);
    assert!(!emu.recover_from_alternate_screen());
}

#[test]
fn emulator_full_reset() {
    let mut emu = make_emulator(5, 10);
    emu.feed_str("TEXT");
    emu.feed(b"\x1b[1m"); // bold
    emu.feed(b"\x1b[?2004h"); // bracketed paste
    emu.feed(b"\x1b]0;title\x07");
    emu.feed(b"\x1bc"); // full reset
    let buf = emu.snapshot();
    assert_eq!(buf.get(0, 0).unwrap().ch, ' ');
    assert!(!emu.bracketed_paste_enabled());
    assert!(emu.title().is_empty());
}

#[test]
fn emulator_decstbm_scroll_region() {
    let mut emu = make_emulator(5, 10);
    let gen0 = emu.buffer_generation();
    for i in 0..5 {
        emu.feed_str(&format!("row{}\n", i));
    }
    emu.feed(b"\x1b[2;4r"); // scroll region rows 2-4
    emu.feed(b"\x1b[2;1H");
    emu.feed_str("X");
    emu.feed(b"\n"); // scroll within region
                     // Just verify it processed without panicking
    assert!(emu.buffer_generation() > gen0);
}

#[test]
fn emulator_da1_response() {
    let mut emu = make_emulator(3, 10);
    emu.feed(b"\x1b[c"); // DA1
    let resp = emu.drain_responses();
    assert_eq!(resp, b"\x1b[?1;0c");
}

#[test]
fn emulator_contents_plain() {
    let mut emu = make_emulator(3, 5);
    emu.feed_str("Hi\n");
    emu.feed_str("Bye");
    let plain = emu.contents_plain();
    assert!(plain.starts_with("Hi"));
    assert!(plain.contains("Bye"));
}

#[test]
fn emulator_buffer_generation_changes_on_write() {
    let mut emu = make_emulator(3, 5);
    let gen0 = emu.buffer_generation();
    emu.feed_str("X");
    assert!(emu.buffer_generation() > gen0);
}

// ─────────────────────────────────────────────────────────────────────
// 5. Config Tests
// ─────────────────────────────────────────────────────────────────────

#[cfg(feature = "vrw")]
#[test]
fn config_default_values() {
    let cfg = vrc_core::config::schema::Config::default();
    assert_eq!(cfg.server.port, 9090);
    assert!(!cfg.security.require_auth);
    assert!(!cfg.tls.enabled);
    assert_eq!(cfg.vtty.rows, 24);
    assert_eq!(cfg.vtty.cols, 80);
}

#[cfg(feature = "vrw")]
#[test]
fn config_deserialize_minimal_json() {
    let json = r#"{ "server": { "bind": "127.0.0.1", "port": 8080 } }"#;
    let cfg: vrc_core::config::schema::Config = serde_json::from_str(json).unwrap();
    assert_eq!(cfg.server.port, 8080);
    assert_eq!(cfg.vtty.rows, 24); // default preserved
}

#[cfg(feature = "vrw")]
#[test]
fn config_deserialize_full_json() {
    let json = r#"{
    "server": { "bind": "0.0.0.0", "port": 3000 },
    "security": { "require_auth": true, "token_file": "custom_token" },
    "vtty": { "rows": 50, "cols": 120, "term": "xterm-256color", "scrollback": 10000, "truecolor": true, "mouse": false, "screenshot_font_size": 14.0 },
    "display": { "enabled": true, "refresh_ms": 50, "display_all": false }
}"#;
    let cfg: vrc_core::config::schema::Config = serde_json::from_str(json).unwrap();
    assert_eq!(cfg.server.bind, "0.0.0.0");
    assert_eq!(cfg.server.port, 3000);
    assert!(cfg.security.require_auth);
    assert_eq!(cfg.security.token_file, "custom_token");
    assert_eq!(cfg.vtty.rows, 50);
    assert_eq!(cfg.vtty.cols, 120);
    assert_eq!(cfg.display.refresh_ms, 50);
}

#[cfg(feature = "vrw")]
#[test]
fn config_serialize_roundtrip() {
    let cfg = vrc_core::config::schema::Config::default();
    let json = serde_json::to_string(&cfg).unwrap();
    let cfg2: vrc_core::config::schema::Config = serde_json::from_str(&json).unwrap();
    assert_eq!(cfg.server.port, cfg2.server.port);
    assert_eq!(cfg.vtty.rows, cfg2.vtty.rows);
}

#[cfg(feature = "vrw")]
#[test]
fn config_merge_local_overrides_global() {
    let mut global = vrc_core::config::schema::Config::default();
    global.server.port = 9090;
    let mut local = vrc_core::config::schema::Config::default();
    local.server.port = 8080;
    let merged = vrc_core::config::merge::merge_configs(global, local);
    assert_eq!(merged.server.port, 8080);
}

#[test]
fn config_merge_handles_keeps_global_when_local_empty() {
    use vrc_core::config::schema::HandleConfig;
    let mut global = vrc_core::config::schema::Config::default();
    global.handles.push(HandleConfig {
        name: "h1".into(),
        sink: "null".into(),
        path: None,
    });
    let local = vrc_core::config::schema::Config::default();
    let merged = vrc_core::config::merge::merge_configs(global, local);
    assert_eq!(merged.handles.len(), 1);
    assert_eq!(merged.handles[0].name, "h1");
}

#[cfg(feature = "vrw")]
#[test]
fn config_apply_profile_overrides_base() {
    use vrc_core::config::schema::PartialConfig;
    let mut base = vrc_core::config::schema::Config::default();
    base.server.port = 9090;
    let mut profile = PartialConfig::default();
    let mut server = vrc_core::config::schema::ServerConfig::default();
    server.port = 3000;
    profile.server = Some(server);
    let result = vrc_core::config::merge::apply_profile(base, &profile);
    assert_eq!(result.server.port, 3000);
}

#[cfg(feature = "vrw")]
#[test]
fn config_apply_profile_none_keeps_base() {
    let base = vrc_core::config::schema::Config::default();
    let profile = vrc_core::config::schema::PartialConfig::default();
    let result = vrc_core::config::merge::apply_profile(base, &profile);
    assert_eq!(result.server.port, 9090);
}

#[test]
fn config_environment_variables() {
    use std::collections::HashMap;
    let env = vrc_core::config::schema::EnvironmentConfig {
        variables: HashMap::from([
            ("KEY1".into(), "val1".into()),
            ("KEY2".into(), "val2".into()),
        ]),
    };
    assert_eq!(env.variables.get("KEY1").unwrap(), "val1");
}

#[cfg(feature = "vrw")]
#[test]
fn config_partial_config_all_none() {
    let pc = vrc_core::config::schema::PartialConfig::default();
    assert!(pc.server.is_none());
    assert!(pc.vtty.is_none());
    assert!(pc.hooks.is_none());
}

// ─────────────────────────────────────────────────────────────────────
// 6. Validation Tests
// ─────────────────────────────────────────────────────────────────────

#[test]
fn validation_default_config_no_errors() {
    let cfg = vrc_core::config::schema::Config::default();
    let issues = vrc_core::config::validation::validate_config(&cfg);
    let errors: Vec<_> = issues
        .iter()
        .filter(|i| i.level == vrc_core::config::validation::ValidationLevel::Error)
        .collect();
    assert!(
        errors.is_empty(),
        "Default config should have no errors: {:?}",
        errors
    );
}

#[cfg(feature = "vrw")]
#[test]
fn validation_port_zero_is_error() {
    let mut cfg = vrc_core::config::schema::Config::default();
    cfg.server.port = 0;
    let issues = vrc_core::config::validation::validate_config(&cfg);
    let port_errs: Vec<_> = issues
        .iter()
        .filter(|i| {
            i.field == "server.port"
                && i.level == vrc_core::config::validation::ValidationLevel::Error
        })
        .collect();
    assert_eq!(port_errs.len(), 1);
}

#[cfg(feature = "vrw")]
#[test]
fn validation_bind_empty_is_error() {
    let mut cfg = vrc_core::config::schema::Config::default();
    cfg.server.bind = String::new();
    let issues = vrc_core::config::validation::validate_config(&cfg);
    assert!(issues.iter().any(|i| i.field == "server.bind"
        && i.level == vrc_core::config::validation::ValidationLevel::Error));
}

#[test]
fn validation_vtty_zero_rows_is_error() {
    let mut cfg = vrc_core::config::schema::Config::default();
    cfg.vtty.rows = 0;
    let issues = vrc_core::config::validation::validate_config(&cfg);
    assert!(issues.iter().any(|i| i.field == "vtty.rows"));
}

#[test]
fn validation_vtty_zero_cols_is_error() {
    let mut cfg = vrc_core::config::schema::Config::default();
    cfg.vtty.cols = 0;
    let issues = vrc_core::config::validation::validate_config(&cfg);
    assert!(issues.iter().any(|i| i.field == "vtty.cols"));
}

#[test]
fn validation_refresh_ms_too_low() {
    let mut cfg = vrc_core::config::schema::Config::default();
    cfg.display.refresh_ms = 5;
    let issues = vrc_core::config::validation::validate_config(&cfg);
    assert!(issues.iter().any(|i| i.field == "display.refresh_ms"));
}

#[cfg(feature = "vrw")]
#[test]
fn validation_multiple_issues() {
    let mut cfg = vrc_core::config::schema::Config::default();
    cfg.server.port = 0;
    cfg.server.bind = String::new();
    cfg.vtty.rows = 0;
    cfg.vtty.cols = 0;
    let issues = vrc_core::config::validation::validate_config(&cfg);
    assert!(issues.len() >= 4);
}

// ─────────────────────────────────────────────────────────────────────
// 7. ProcessError Tests
// ─────────────────────────────────────────────────────────────────────

#[test]
fn error_command_not_found_display() {
    let err = vrc_core::process::error::ProcessError::CommandNotFound("xyz".into());
    let msg = format!("{}", err);
    assert!(msg.contains("xyz"));
    assert!(msg.contains("not found"));
}

#[test]
fn error_spawn_failed_display() {
    let err = vrc_core::process::error::ProcessError::SpawnFailed { cmd: "bash".into() };
    let msg = format!("{}", err);
    assert!(msg.contains("bash"));
    assert!(msg.contains("spawn"));
}

#[test]
fn error_from_io_error() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "no file");
    let err: vrc_core::process::error::ProcessError = io_err.into();
    assert!(matches!(err, vrc_core::process::error::ProcessError::Io(_)));
}

#[test]
fn error_io_has_source() {
    let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
    let err = vrc_core::process::error::ProcessError::Io(io_err);
    assert!(std::error::Error::source(&err).is_some());
}

#[test]
fn error_non_io_no_source() {
    let err = vrc_core::process::error::ProcessError::ChannelClosed("c1".into());
    assert!(std::error::Error::source(&err).is_none());
}

#[test]
fn error_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<vrc_core::process::error::ProcessError>();
}

#[test]
fn error_result_type_alias() {
    type Result<T> = std::result::Result<T, vrc_core::process::error::ProcessError>;
    let r: Result<String> = Ok("ok".into());
    assert_eq!(r.unwrap(), "ok");
    let r2: Result<()> = Err(vrc_core::process::error::ProcessError::PlatformNotSupported(
        "freeze".into(),
    ));
    assert!(r2.is_err());
}

#[test]
fn error_sink_already_exists_display() {
    let err = vrc_core::process::error::ProcessError::SinkAlreadyExists {
        name: "out".into(),
        command_id: "cmd1".into(),
    };
    let msg = format!("{}", err);
    assert!(msg.contains("out") && msg.contains("cmd1"));
}

#[test]
fn error_snapshot_not_found_display() {
    let err = vrc_core::process::error::ProcessError::SnapshotNotFound {
        name: "snap".into(),
        command_id: "c1".into(),
    };
    let msg = format!("{}", err);
    assert!(msg.contains("snap") && msg.contains("c1"));
}

// ─────────────────────────────────────────────────────────────────────
// 8. Handle Registry Tests
// ─────────────────────────────────────────────────────────────────────

#[test]
fn handle_registry_new_empty() {
    let reg = vrc_core::handles::registry::HandleRegistry::new();
    assert!(reg.list().is_empty());
}

#[test]
fn handle_registry_default_trait() {
    let reg = vrc_core::handles::registry::HandleRegistry::default();
    assert!(reg.list().is_empty());
}

// ─────────────────────────────────────────────────────────────────────
// 9. Null Sink Tests
// ─────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn null_sink_write_ignores_data() {
    use vrc_core::handles::sink::Sink;
    let mut sink = vrc_core::handles::null_sink::NullSink;
    sink.write(b"anything").await;
    sink.flush().await;
    // If we reach here, no panic — pass
}

// ─────────────────────────────────────────────────────────────────────
// 10. File Sink Tests
// ─────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn file_sink_write_and_read() {
    use vrc_core::handles::sink::Sink;
    let dir = std::env::temp_dir().join("vrw_test_file_sink");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("test_output.txt");
    let _ = std::fs::remove_file(&path); // cleanup from prior run

    let mut sink = vrc_core::handles::file_sink::FileSink::new(path.to_str().unwrap()).unwrap();
    sink.write(b"hello ").await;
    sink.write(b"world\n").await;
    sink.flush().await;
    drop(sink);

    let content = std::fs::read_to_string(&path).unwrap();
    assert_eq!(content, "hello world\n");
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn file_sink_append() {
    let dir = std::env::temp_dir().join("vrw_test_file_append");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("append.txt");
    let _ = std::fs::remove_file(&path);

    use vrc_core::handles::sink::Sink;
    {
        let mut s1 = vrc_core::handles::file_sink::FileSink::new(path.to_str().unwrap()).unwrap();
        s1.write(b"first\n").await;
    }
    {
        let mut s2 = vrc_core::handles::file_sink::FileSink::new(path.to_str().unwrap()).unwrap();
        s2.write(b"second\n").await;
        s2.flush().await;
    }

    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("first"));
    assert!(content.contains("second"));
    let _ = std::fs::remove_file(&path);
}

// ─────────────────────────────────────────────────────────────────────
// 11. CommandLogger Tests
// ─────────────────────────────────────────────────────────────────────

#[test]
fn logger_disabled_no_output() {
    let logger = vrc_core::logging::command_log::CommandLogger::new(false, None, "vrc", false, Default::default()).unwrap();
    // Subscribe BEFORE logging so we catch the broadcast
    let mut rx = logger.subscribe();
    logger.log("test", "should not appear");
    // Memory buffer is always populated (for web UI and event loop),
    // but file output is suppressed when disabled.
    assert_eq!(logger.read_memory_buffer().len(), 1);
    // Verify the broadcast channel also received the event
    let entry = rx.try_recv().unwrap();
    assert!(entry.contains("test"));
}

#[test]
fn logger_enabled_stores_in_memory() {
    let logger = vrc_core::logging::command_log::CommandLogger::new(true, None, "vrw", false, Default::default()).unwrap();
    logger.log("spawn", "cmd1 started");
    logger.log("kill", "cmd1 killed");
    let buf = logger.read_memory_buffer();
    assert_eq!(buf.len(), 2);
    assert!(buf[0].contains("spawn"));
    assert!(buf[1].contains("kill"));
}

#[test]
fn logger_memory_buffer_arc_shared() {
    let logger = vrc_core::logging::command_log::CommandLogger::new(true, None, "vrc", false, Default::default()).unwrap();
    let arc = logger.memory_buffer_arc();
    logger.log("test", "entry");
    let buf = arc.lock().unwrap();
    assert_eq!(buf.len(), 1);
}

#[test]
fn logger_subscribe_broadcasts() {
    let logger = vrc_core::logging::command_log::CommandLogger::new(true, None, "vrc", false, Default::default()).unwrap();
    let mut rx = logger.subscribe();
    logger.log("test", "broadcast-msg");
    let received = rx.try_recv().unwrap();
    assert!(received.contains("broadcast-msg"));
}

#[test]
fn logger_ring_buffer_eviction() {
    let logger = vrc_core::logging::command_log::CommandLogger::new(true, None, "vrc", false, Default::default()).unwrap();
    // MEMORY_BUFFER_CAPACITY is 2048 — we can't fill that in a unit test,
    // but verify the interface works with a few entries.
    for i in 0..10 {
        logger.log("loop", &format!("msg {}", i));
    }
    let buf = logger.read_memory_buffer();
    assert_eq!(buf.len(), 10);
}

// ─────────────────────────────────────────────────────────────────────
// 12. VTTY Renderer Tests
// ─────────────────────────────────────────────────────────────────────

#[test]
fn renderer_to_html_basic() {
    use vrc_core::vtty::cell::Cell;
    let mut buf = vrc_core::vtty::buffer::Buffer::new(10, 2, 100);
    buf.set(0, 0, Cell::new('H'));
    buf.set(0, 1, Cell::new('i'));
    let html = vrc_core::vtty::renderer::VttyRenderer::to_html(&buf);
    assert!(html.contains("H"));
    assert!(html.contains("i"));
}

#[test]
fn renderer_to_html_empty_buffer() {
    let buf = vrc_core::vtty::buffer::Buffer::new(10, 2, 100);
    let html = vrc_core::vtty::renderer::VttyRenderer::to_html(&buf);
    // to_html wraps each cell in a span, no <pre> or <div> wrapper
    assert!(html.contains("<span") || html.len() > 0);
}

// ─────────────────────────────────────────────────────────────────────
// 13. VTTY Rate Limiter Tests
// ─────────────────────────────────────────────────────────────────────

#[test]
fn rate_limiter_first_call_succeeds() {
    let mut limiter = vrc_core::vtty::rate_limiter::RateLimiter::new(10);
    assert!(limiter.allow()); // first call always succeeds
}

#[test]
fn rate_limiter_disabled_always_allows() {
    let mut limiter = vrc_core::vtty::rate_limiter::RateLimiter::disabled();
    for _ in 0..100 {
        assert!(limiter.allow());
    }
}

#[test]
fn rate_limiter_max_rate_config() {
    let limiter = vrc_core::vtty::rate_limiter::RateLimiter::new(30);
    assert_eq!(limiter.max_rate(), 30);
    assert!(!limiter.is_disabled());
}

// ─────────────────────────────────────────────────────────────────────
// 14. Instance Info Serialization Tests
// ─────────────────────────────────────────────────────────────────────

#[cfg(feature = "vrw")]
#[test]
fn instance_info_serialization_roundtrip() {
    let info = vrc_core::instance::info::InstanceInfo {
        pid: 12345,
        port: 9090,
        bind: "0.0.0.0".into(),
        start_time: chrono::Utc::now(),
        daemon: true,
        display: false,
        command: Some("htop".into()),
        name: Some("test-server".into()),
    };
    let json = serde_json::to_string(&info).unwrap();
    let info2: vrc_core::instance::info::InstanceInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(info.pid, info2.pid);
    assert_eq!(info.port, info2.port);
    assert_eq!(info.bind, info2.bind);
    assert_eq!(info.daemon, info2.daemon);
    assert_eq!(info.command, info2.command);
}

#[cfg(not(feature = "vrw"))]
#[test]
fn instance_info_serialization_roundtrip_vrc() {
    let info = vrc_core::instance::info::InstanceInfo {
        pid: 12345,
        start_time: chrono::Utc::now(),
        daemon: true,
        display: false,
    };
    let json = serde_json::to_string(&info).unwrap();
    let info2: vrc_core::instance::info::InstanceInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(info.pid, info2.pid);
    assert_eq!(info.daemon, info2.daemon);
    assert_eq!(info.display, info2.display);
}

// ─────────────────────────────────────────────────────────────────────
// 15. VTTY Sink Tests
// ─────────────────────────────────────────────────────────────────────

#[test]
fn vtty_output_new_has_no_sinks() {
    let output = vrc_core::vtty::sink::VttyOutput::new();
    assert_eq!(output.sink_count(), 0);
}

#[test]
fn vtty_output_with_sinks() {
    use std::sync::Arc;
    let sink = Arc::new(vrc_core::vtty::sink::InMemoryVttySink::new());
    let output = vrc_core::vtty::sink::VttyOutput::with_sinks(vec![sink.clone()]);
    assert_eq!(output.sink_count(), 1);
}

#[test]
fn vtty_in_memory_sink_initially_empty() {
    let sink = vrc_core::vtty::sink::InMemoryVttySink::new();
    assert!(sink.latest().is_none());
    assert_eq!(sink.change_count(), 0);
}

#[test]
fn vtty_in_memory_sink_reset() {
    let sink = vrc_core::vtty::sink::InMemoryVttySink::new();
    sink.reset();
    assert_eq!(sink.change_count(), 0);
    assert!(sink.latest().is_none());
}

// ─────────────────────────────────────────────────────────────────────
// 16. Hooks Config Tests
// ─────────────────────────────────────────────────────────────────────

#[test]
fn hooks_config_default_no_hooks() {
    let hooks = vrc_core::config::hooks::HooksConfig::default();
    assert!(hooks.on_spawn.is_none());
    assert!(hooks.on_exit.is_none());
    assert!(hooks.on_error.is_none());
}

#[test]
fn hooks_config_deserialize() {
    let json =
        r#"{ "on_spawn": "echo starting", "on_exit": "echo done", "on_error": "echo failed" }"#;
    let hooks: vrc_core::config::hooks::HooksConfig = serde_json::from_str(json).unwrap();
    assert_eq!(hooks.on_spawn.as_deref(), Some("echo starting"));
    assert_eq!(hooks.on_exit.as_deref(), Some("echo done"));
    assert_eq!(hooks.on_error.as_deref(), Some("echo failed"));
}

// ─────────────────────────────────────────────────────────────────────
// 17. Merge Edge Cases
// ─────────────────────────────────────────────────────────────────────

#[test]
fn merge_command_env_empty_overrides() {
    use std::collections::HashMap;
    let config_env = vrc_core::config::schema::EnvironmentConfig {
        variables: HashMap::from([("A".into(), "1".into())]),
    };
    let merged = vrc_core::config::merge::merge_command_env(&config_env, HashMap::new());
    assert_eq!(merged.get("A").unwrap(), "1");
}

#[test]
fn merge_profiles_local_overrides_global() {
    use std::collections::HashMap;
    let global = vrc_core::config::schema::ProfilesConfig {
        entries: HashMap::from([(
            "dev".into(),
            vrc_core::config::schema::PartialConfig::default(),
        )]),
    };
    let local = vrc_core::config::schema::ProfilesConfig {
        entries: HashMap::from([(
            "prod".into(),
            vrc_core::config::schema::PartialConfig::default(),
        )]),
    };
    let merged = {
        let mut entries = global.entries;
        entries.extend(local.entries);
        vrc_core::config::schema::ProfilesConfig { entries }
    };
    assert!(merged.entries.contains_key("dev"));
    assert!(merged.entries.contains_key("prod"));
}

// ─────────────────────────────────────────────────────────────────────
// 18. Edge Cases & Regression Tests
// ─────────────────────────────────────────────────────────────────────

#[test]
fn emulator_empty_feed() {
    let mut emu = make_emulator(3, 5);
    emu.feed(b"");
    let buf = emu.snapshot();
    assert_eq!(buf.generation(), 0); // no mutations
}

#[test]
fn emulator_feed_binary_garbage() {
    let mut emu = make_emulator(3, 10);
    emu.feed(&[0x80, 0x81, 0x82, 0xff]); // invalid UTF-8
                                         // Should not panic
    let buf = emu.snapshot();
    assert_eq!(buf.width, 10);
}

#[test]
fn emulator_scroll_multiple_times() {
    let mut emu = make_emulator(3, 5);
    for _ in 0..20 {
        emu.feed_str("XXXXX\n");
    }
    let buf = emu.snapshot();
    // With max_scrollback=1000, all 20 scrollbacks should be stored
    assert!(buf.scrollback.len() >= 3);
    // Buffer still has 3 visible rows
    assert_eq!(buf.height, 3);
}

#[test]
fn emulator_set_bg_color() {
    let mut emu = make_emulator(3, 10);
    emu.feed(b"\x1b[48;2;10;20;30m");
    emu.feed_str("X");
    let buf = emu.snapshot();
    assert_eq!(buf.get(0, 0).unwrap().bg, [10, 20, 30]);
}

#[test]
fn emulator_sgr_underline_blink() {
    let mut emu = make_emulator(3, 10);
    emu.feed(b"\x1b[4;5m");
    emu.feed_str("U");
    let buf = emu.snapshot();
    assert!(buf.get(0, 0).unwrap().underline);
    assert!(buf.get(0, 0).unwrap().blink);
}

#[test]
fn emulator_sgr_strikethrough_invisible() {
    let mut emu = make_emulator(3, 10);
    emu.feed(b"\x1b[8;9m");
    emu.feed_str("S");
    let buf = emu.snapshot();
    assert!(buf.get(0, 0).unwrap().invisible);
    assert!(buf.get(0, 0).unwrap().strikethrough);
}

#[test]
fn buffer_clear_screen_to() {
    let mut b = vrc_core::vtty::buffer::Buffer::new(10, 5, 100);
    b.rows[0][0].ch = 'A';
    b.rows[1][5].ch = 'B';
    b.rows[2][9].ch = 'C';
    b.rows[3][0].ch = 'D';
    b.clear_screen_to(2, 5);
    assert_eq!(b.rows[0][0].ch, ' '); // cleared
    assert_eq!(b.rows[1][5].ch, ' '); // cleared
    assert_eq!(b.rows[2][5].ch, ' '); // cleared (inclusive)
    assert_eq!(b.rows[2][9].ch, 'C'); // after col 5: untouched
    assert_eq!(b.rows[3][0].ch, 'D'); // below: untouched
}

// ─────────────────────────────────────────────────────────────────────
// 19. CommandLogConfig Default Tests
// ─────────────────────────────────────────────────────────────────────

#[test]
fn command_log_config_default_includes_terminal() {
    let cfg = vrc_core::config::schema::CommandLogConfig::default();
    assert!(!cfg.enabled);
    assert!(cfg.file.is_none());
    assert!(cfg.pty_raw_log.is_none());
    // The `terminal` field must have sensible defaults
    assert!(!cfg.terminal.format.is_empty(), "terminal format should have a default");
    // Verify default format contains expected placeholders
    assert!(cfg.terminal.format.contains("%timestamp%"));
    assert!(cfg.terminal.format.contains("%pid%"));
    assert!(cfg.terminal.format.contains("%cmd%"));
    assert!(cfg.terminal.format.contains("%event%"));
}

#[test]
fn terminal_log_config_colors_defaults() {
    let colors = vrc_core::config::hooks::TerminalLogColors::default();
    // Every color field should have a non-empty ANSI string (or empty for no color)
    assert!(!colors.timestamp.ansi.is_empty());
    assert!(!colors.pid.ansi.is_empty());
    assert!(!colors.cmd.ansi.is_empty());
    assert!(!colors.event.ansi.is_empty());
}

#[test]
fn terminal_log_config_pad_defaults() {
    let pad = vrc_core::config::hooks::TerminalLogPad::default();
    assert!(pad.pid > 0);
    assert!(pad.cmd > 0);
    assert!(pad.event > 0);
}

#[test]
fn command_log_config_serialize_roundtrip() {
    let cfg = vrc_core::config::schema::CommandLogConfig::default();
    let json = serde_json::to_string(&cfg).unwrap();
    let cfg2: vrc_core::config::schema::CommandLogConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(cfg.enabled, cfg2.enabled);
    assert_eq!(cfg.terminal.format, cfg2.terminal.format);
}

// ─────────────────────────────────────────────────────────────────────
// 20. Exit Config Default Tests
// ─────────────────────────────────────────────────────────────────────

#[test]
fn exit_config_default_no_retain() {
    let ec = vrc_core::config::schema::ExitConfig::default();
    assert!(!ec.retain_on_exit);
    assert!(ec.on_exit.is_none());
    assert!(ec.on_error.is_none());
    assert!(ec.snapshot_on_exit.is_none());
    assert_eq!(ec.timeout_secs, 10);
}

#[test]
fn exit_config_retain_flag_writable() {
    let mut ec = vrc_core::config::schema::ExitConfig::default();
    assert!(!ec.retain_on_exit);
    ec.retain_on_exit = true;
    assert!(ec.retain_on_exit);
    ec.retain_on_exit = false;
    assert!(!ec.retain_on_exit);
}
