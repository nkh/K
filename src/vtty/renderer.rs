use super::buffer::Buffer;

/// Format an RGB triplet as a hex color string: "#RRGGBB".
#[inline]
fn hex_color(c: [u8; 3]) -> [u8; 7] {
    let mut out = [b'#', 0, 0, 0, 0, 0, 0];
    let hex = |b: u8| -> (u8, u8) {
        let h = b >> 4;
        let l = b & 0x0f;
        (if h < 10 { b'0' + h } else { b'a' + h - 10 }, if l < 10 { b'0' + l } else { b'a' + l - 10 })
    };
    let (h0, l0) = hex(c[0]);
    let (h1, l1) = hex(c[1]);
    let (h2, l2) = hex(c[2]);
    out[1] = h0; out[2] = l0; out[3] = h1; out[4] = l1; out[5] = h2; out[6] = l2;
    out
}

/// Renders a VTTY buffer to various output formats.
pub struct VttyRenderer;

impl VttyRenderer {
    /// Serialize buffer back to ANSI text with color codes.
    pub fn to_ansi(buffer: &Buffer) -> String {
        let mut output = String::new();
        let mut last_fg: Option<[u8; 3]> = None;
        let mut last_bg: Option<[u8; 3]> = None;
        let mut last_bold = false;
        let mut last_italic = false;
        let mut last_underline = false;
        let mut last_reverse = false;
        let mut last_strikethrough = false;

        for row in &buffer.rows {
            for cell in row {
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
                if cell.reverse != last_reverse {
                    codes.push(if cell.reverse { "7" } else { "27" }.to_string());
                    last_reverse = cell.reverse;
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
            output.push('\n');
        }

        output.push_str("\x1b[0m");
        output
    }

    /// Serialize buffer to HTML with run-length encoding.
    ///
    /// Consecutive cells with identical foreground, background, and decoration
    /// flags are merged into a single `<span>` element. This dramatically
    /// reduces the number of DOM nodes (e.g., 10,000 → ~2,000 for a typical
    /// terminal), which is the primary bottleneck for initial page load.
    ///
    /// Uses CSS class `c` (defined in style.css) for the base cell styling.
    /// Colors use hex format (#RRGGBB) instead of rgb() for compactness.
    /// The `<pre>` element uses a monospace font, so cells render at uniform
    /// width without explicit `width:1ch` per element — this avoids creating
    /// per-element block formatting contexts (the dominant layout cost).
    ///
    /// Returns the inner content only (no outer `<pre>` wrapper).
    pub fn to_html(buffer: &Buffer) -> String {
        let total_cells = buffer.rows.len() * buffer.width;
        // RLE estimate: assume average run of 5 cells, each producing ~35 chars.
        // Fallback: if RLE doesn't help, the output is similar to old size.
        let mut html = String::with_capacity(total_cells * 20);
        let mut run_text: String = String::with_capacity(32);

        let width = buffer.width;
        for row in &buffer.rows {
            // Cap at buffer.width to prevent rendering extra cells if a row
            // somehow has more cells than expected (e.g., race during resize).
            let row_len = row.len().min(width);
            if row_len == 0 {
                html.push('\n');
                continue;
            }
            let mut i = 0;
            while i < row_len {
                let cell = &row[i];
                // Start a new run with this cell's style
                let fg = if cell.reverse { cell.bg } else { cell.fg };
                let bg = if cell.reverse { cell.fg } else { cell.bg };
                let fg_hex = hex_color(fg);
                let bg_hex = hex_color(bg);
                let bold = cell.bold;
                let italic = cell.italic;
                let underline = cell.underline;
                let strikethrough = cell.strikethrough;
                let blink = cell.blink;

                // Accumulate characters that share this exact style.
                // Width-0 cells (wide-char continuations) use U+200B so they
                // contribute zero visual columns; regular empty cells use a
                // plain space so they occupy exactly 1ch in the monospace font
                // and preserve column alignment.
                run_text.clear();
                let ch = if cell.width == 0 { '\u{200b}' } else if cell.is_empty() { ' ' } else { cell.ch };
                run_text.push(ch);
                let mut j = i + 1;
                while j < row_len {
                    let next = &row[j];
                    let nfg = if next.reverse { next.bg } else { next.fg };
                    let nbg = if next.reverse { next.fg } else { next.bg };
                    if nfg != fg || nbg != bg || next.bold != bold
                        || next.italic != italic || next.underline != underline
                        || next.strikethrough != strikethrough || next.blink != blink
                    {
                        break;
                    }
                    let nch = if next.width == 0 { '\u{200b}' } else if next.is_empty() { ' ' } else { next.ch };
                    run_text.push(nch);
                    j += 1;
                }

                // Build style string
                html.push_str("<span class=\"c\" style=\"color:");
                html.push_str(std::str::from_utf8(&fg_hex).unwrap());
                html.push_str(";background:");
                html.push_str(std::str::from_utf8(&bg_hex).unwrap());
                if bold {
                    html.push_str(";font-weight:bold");
                }
                if italic {
                    html.push_str(";font-style:italic");
                }
                if underline && strikethrough {
                    html.push_str(";text-decoration:underline line-through");
                } else if underline {
                    html.push_str(";text-decoration:underline");
                } else if strikethrough {
                    html.push_str(";text-decoration:line-through");
                }
                if blink {
                    html.push_str(";animation:blink 1s step-end infinite");
                }
                html.push_str("\">");
                // Push run text with HTML escaping
                for ch in run_text.chars() {
                    match ch {
                        '&' => html.push_str("&amp;"),
                        '<' => html.push_str("&lt;"),
                        '>' => html.push_str("&gt;"),
                        '\'' => html.push_str("&#39;"),
                        '"' => html.push_str("&quot;"),
                        c => html.push(c),
                    }
                }
                html.push_str("</span>");
                i = j;
            }
            html.push('\n');
        }

        html
    }

    /// Serialize buffer (including scrollback) to HTML with RLE encoding.
    /// Same format as to_html() but includes scrollback lines.
    pub fn to_html_scrollback(
        buffer: &Buffer,
        scrollback_offset: usize,
        visible_rows: usize,
    ) -> String {
        let total_lines = buffer.total_lines();
        let max_offset = total_lines.saturating_sub(visible_rows);
        let effective_offset = scrollback_offset.min(max_offset);

        let mut html = String::with_capacity(visible_rows * buffer.width * 20);
        let mut run_text: String = String::with_capacity(32);

        let all_lines: Vec<&Vec<super::cell::Cell>> = buffer
            .scrollback
            .iter()
            .chain(buffer.rows.iter())
            .skip(effective_offset)
            .take(visible_rows)
            .collect();

        let width = buffer.width;
        for row in &all_lines {
            let row_len = row.len().min(width);
            if row_len == 0 {
                html.push('\n');
                continue;
            }
            let mut i = 0;
            while i < row_len {
                let cell = &row[i];
                let fg = if cell.reverse { cell.bg } else { cell.fg };
                let bg = if cell.reverse { cell.fg } else { cell.bg };
                let fg_hex = hex_color(fg);
                let bg_hex = hex_color(bg);
                let bold = cell.bold;
                let italic = cell.italic;
                let underline = cell.underline;
                let strikethrough = cell.strikethrough;
                let blink = cell.blink;

                run_text.clear();
                // Width-0 cells (wide-char continuations) use U+200B so they
                // contribute zero visual columns; regular empty cells use a
                // plain space so they occupy exactly 1ch in the monospace font
                // and preserve column alignment.
                let ch = if cell.width == 0 { '\u{200b}' } else if cell.is_empty() { ' ' } else { cell.ch };
                run_text.push(ch);
                let mut j = i + 1;
                while j < row_len {
                    let next = &row[j];
                    let nfg = if next.reverse { next.bg } else { next.fg };
                    let nbg = if next.reverse { next.fg } else { next.bg };
                    if nfg != fg || nbg != bg || next.bold != bold
                        || next.italic != italic || next.underline != underline
                        || next.strikethrough != strikethrough || next.blink != blink
                    {
                        break;
                    }
                    let nch = if next.width == 0 { '\u{200b}' } else if next.is_empty() { ' ' } else { next.ch };
                    run_text.push(nch);
                    j += 1;
                }

                html.push_str("<span class=\"c\" style=\"color:");
                html.push_str(std::str::from_utf8(&fg_hex).unwrap());
                html.push_str(";background:");
                html.push_str(std::str::from_utf8(&bg_hex).unwrap());
                if bold { html.push_str(";font-weight:bold"); }
                if italic { html.push_str(";font-style:italic"); }
                if underline && strikethrough {
                    html.push_str(";text-decoration:underline line-through");
                } else if underline {
                    html.push_str(";text-decoration:underline");
                } else if strikethrough {
                    html.push_str(";text-decoration:line-through");
                }
                if blink { html.push_str(";animation:blink 1s step-end infinite"); }
                html.push_str("\">");
                for ch in run_text.chars() {
                    match ch {
                        '&' => html.push_str("&amp;"),
                        '<' => html.push_str("&lt;"),
                        '>' => html.push_str("&gt;"),
                        '\'' => html.push_str("&#39;"),
                        '"' => html.push_str("&quot;"),
                        c => html.push(c),
                    }
                }
                html.push_str("</span>");
                i = j;
            }
            html.push('\n');
        }

        html
    }

    /// Serialize buffer to plain text (no formatting).
    pub fn to_plain(buffer: &Buffer) -> String {
        buffer
            .rows
            .iter()
            .map(|row| row.iter().map(|c| c.ch).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Get a range of lines as plain text.
    pub fn lines_plain(buffer: &Buffer, start: usize, count: usize) -> Vec<String> {
        buffer
            .rows
            .iter()
            .skip(start)
            .take(count)
            .map(|row| row.iter().map(|c| c.ch).collect::<String>())
            .collect()
    }

    /// Get a range of lines including scrollback.
    pub fn lines_with_scrollback(buffer: &Buffer, start: usize, count: usize) -> Vec<String> {
        let all_lines: Vec<_> = buffer
            .scrollback
            .iter()
            .chain(buffer.rows.iter())
            .skip(start)
            .take(count)
            .map(|row| row.iter().map(|c| c.ch).collect::<String>())
            .collect();
        all_lines
    }
}

#[cfg(test)]
mod tests {
    use super::super::cell::Cell;
    use super::*;

    #[test]
    fn test_to_plain() {
        let mut buf = Buffer::new(5, 3, 100);
        buf.rows[0][0].ch = 'H';
        buf.rows[0][1].ch = 'i';
        let text = VttyRenderer::to_plain(&buf);
        assert!(text.starts_with("Hi   "));
    }

    #[test]
    fn test_to_ansi() {
        let mut buf = Buffer::new(5, 2, 100);
        buf.rows[0][0].ch = 'A';
        buf.rows[0][0].fg = [255, 0, 0];
        let ansi = VttyRenderer::to_ansi(&buf);
        assert!(ansi.contains("38;2;255;0;0"));
        assert!(ansi.contains("A"));
        assert!(ansi.ends_with("\x1b[0m"));
    }

    #[test]
    fn test_to_html() {
        let mut buf = Buffer::new(5, 2, 100);
        buf.rows[0][0].ch = 'X';
        buf.rows[0][0].fg = [0, 255, 0];
        let html = VttyRenderer::to_html(&buf);
        // Uses hex color format (#00ff00) and CSS class "c"
        assert!(html.contains("#00ff00"), "expected hex color for green fg");
        assert!(html.contains("class=\"c\""), "expected CSS class 'c'");
        assert!(html.contains("X"));
        // Should NOT contain old rgb() format or inline-block
        assert!(!html.contains("rgb("), "should not use rgb() format");
        assert!(!html.contains("inline-block"), "should not use inline-block");
    }

    #[test]
    fn test_html_escape_metacharacters() {
        let mut buf = Buffer::new(10, 1, 100);
        buf.rows[0][0].ch = '<';
        buf.rows[0][1].ch = '>';
        buf.rows[0][2].ch = '&';
        buf.rows[0][3].ch = '\'';
        buf.rows[0][4].ch = '"';
        let html = VttyRenderer::to_html(&buf);
        // Each metachar must be properly escaped in the output
        assert!(html.contains("&lt;"), "expected escaped <");
        assert!(html.contains("&gt;"), "expected escaped >");
        assert!(html.contains("&amp;"), "expected escaped &");
        assert!(html.contains("&#39;"), "expected escaped '");
        assert!(html.contains("&quot;"), "expected escaped \"");
        // The raw & should appear exactly as part of the five escaped entities
        // (5 cells each starting with &).  If any cell were not escaped, we'd
        // see extra bare & characters.
        let amp_count = html.matches('&').count();
        assert_eq!(
            amp_count, 5,
            "expected exactly 5 & chars (one per escaped entity), got {}",
            amp_count
        );
    }

    #[test]
    fn test_lines_with_scrollback() {
        let mut buf = Buffer::new(5, 2, 100);
        buf.scrollback.push(vec![Cell::new('S')]);
        buf.rows[0][0].ch = 'V';
        let lines = VttyRenderer::lines_with_scrollback(&buf, 0, 10);
        assert_eq!(lines.len(), 3); // 1 scrollback + 2 visible
        assert!(lines[0].starts_with('S'));
        assert!(lines[1].starts_with('V'));
    }

    // ─── RLE wide-char rendering tests ───

    #[test]
    fn test_rle_wide_char_uses_zero_width_space() {
        // A wide character (width=2) followed by its continuation (width=0)
        // should render the continuation as U+200B (zero-width space) to
        // preserve column alignment in the monospace <pre>.
        let mut buf = Buffer::new(6, 1, 100);
        buf.rows[0][2].ch = '你';
        buf.rows[0][2].width = 2;
        buf.rows[0][3].ch = ' '; // continuation placeholder
        buf.rows[0][3].width = 0;
        buf.rows[0][4].ch = 'Z';
        let html = VttyRenderer::to_html(&buf);
        // The continuation cell should produce a zero-width space, not a normal space
        assert!(html.contains('\u{200b}'), "wide-char continuation should render as U+200B");
        // The wide char itself should appear in the output
        assert!(html.contains('你'), "wide char should be in output");
        // The Z after the wide char should also appear
        assert!(html.contains('Z'), "trailing char should be in output");
    }

    #[test]
    fn test_rle_wide_char_continuation_different_style() {
        // When a wide char and its continuation have different styles,
        // they should be rendered as separate spans (not merged).
        let mut buf = Buffer::new(6, 1, 100);
        buf.rows[0][0].ch = 'A';
        buf.rows[0][0].fg = [100, 100, 100]; // grey
        buf.rows[0][1].ch = '你';
        buf.rows[0][1].width = 2;
        buf.rows[0][1].fg = [255, 0, 0]; // red
        buf.rows[0][2].ch = ' '; // continuation
        buf.rows[0][2].width = 0;
        buf.rows[0][2].fg = [0, 0, 255]; // blue — different from lead!
        buf.rows[0][3].ch = 'B';
        buf.rows[0][3].fg = [100, 100, 100]; // grey — same as A
        let html = VttyRenderer::to_html(&buf);
        // Should have at least 4 spans: A(span1), 你(span2), cont(span3), B(merged with A or new)
        let span_count = html.matches("<span class=\"c\"").count();
        assert!(span_count >= 3, "wide char with different continuation style should produce separate spans, got {}", span_count);
        // Both 你 and the zero-width space should be present
        assert!(html.contains('你'));
        assert!(html.contains('\u{200b}'));
    }

    #[test]
    fn test_rle_empty_cells_use_space_not_zwsp() {
        // Normal empty cells (width=1, default attributes) should render as
        // regular spaces, NOT as zero-width spaces.  This is the core of the
        // column-alignment bug — if empty cells become U+200B, columns collapse.
        let mut buf = Buffer::new(5, 1, 100);
        buf.rows[0][0].ch = 'X';
        buf.rows[0][0].fg = [255, 0, 0];
        // Cells 1-3 are default (empty: ch=' ', width=1)
        buf.rows[0][4].ch = 'Y';
        buf.rows[0][4].fg = [255, 0, 0];
        let html = VttyRenderer::to_html(&buf);
        // Count zero-width spaces — should be exactly 0 (no wide chars in this buffer)
        let zwsp_count = html.matches('\u{200b}').count();
        assert_eq!(zwsp_count, 0, "normal empty cells should NOT produce U+200B, got {}", zwsp_count);
        // The X and Y should be present
        assert!(html.contains('X'));
        assert!(html.contains('Y'));
    }

    #[test]
    fn test_rle_mixed_wide_and_narrow() {
        // A realistic scenario: mixed wide and narrow characters on one row.
        // Row: "A你 B" where 你 is width=2
        // Columns: 0=A(w1), 1=你(w2), 2=cont(w0), 3=space(w1), 4=B(w1)
        let mut buf = Buffer::new(5, 1, 100);
        buf.rows[0][0].ch = 'A';
        buf.rows[0][0].fg = [200, 200, 200];
        buf.rows[0][1].ch = '你';
        buf.rows[0][1].width = 2;
        buf.rows[0][1].fg = [200, 200, 200];
        buf.rows[0][2].ch = ' ';
        buf.rows[0][2].width = 0;
        buf.rows[0][2].fg = [200, 200, 200];
        // Cell 3 is default empty
        buf.rows[0][4].ch = 'B';
        buf.rows[0][4].fg = [200, 200, 200];
        let html = VttyRenderer::to_html(&buf);
        // All same style, so should merge into a single span
        assert!(html.contains('A'));
        assert!(html.contains('你'));
        assert!(html.contains('\u{200b}')); // continuation
        assert!(html.contains('B'));
        // Verify exactly one zero-width space (one wide char continuation)
        let zwsp_count = html.matches('\u{200b}').count();
        assert_eq!(zwsp_count, 1, "should have exactly 1 ZWSP for 1 wide char");
    }

    #[test]
    fn test_rle_row_produces_correct_newline() {
        // Each row should end with exactly one newline
        let mut buf = Buffer::new(3, 2, 100);
        buf.rows[0][0].ch = 'A';
        buf.rows[1][0].ch = 'B';
        let html = VttyRenderer::to_html(&buf);
        let lines: Vec<&str> = html.split('\n').collect();
        // 2 rows + trailing empty from final newline = 3 entries, last empty
        assert_eq!(lines.len(), 3, "2-row buffer should produce 3 split entries (2 + trailing)");
        assert!(lines[0].contains('A'));
        assert!(lines[1].contains('B'));
        assert_eq!(lines[2], "", "trailing newline should produce empty final entry");
    }

    // ─── Performance benchmarks ───

    /// Populate a buffer with realistic terminal content (mixed chars, colors, styles).
    fn populate_buffer(buf: &mut Buffer) {
        let chars: &[char] = &[
            'A', 'B', 'C', 'a', 'b', 'c', '0', '1', '2', '3',
            ' ', ' ', ' ', '>', '<', '|', '-', '_', '=', '+',
            '#', '@', '!', '$', '%', '^', '&', '*', '(', ')',
            '/', '\\', ':', ';', '.', ',', '[', ']', '{', '}',
            '~', '`', '\'', '"', '\t', 'X', 'Y', 'Z', 'W',
        ];
        let colors: &[[u8; 3]] = &[
            [196, 50, 50], [50, 196, 50], [50, 50, 196],
            [196, 196, 50], [196, 50, 196], [50, 196, 196],
            [200, 200, 200], [100, 100, 100], [0, 0, 0],
        ];
        for (row_idx, row) in buf.rows.iter_mut().enumerate() {
            for (col_idx, cell) in row.iter_mut().enumerate() {
                let ci = (row_idx * buf.width + col_idx) % chars.len();
                cell.ch = chars[ci];
                let fg_i = (row_idx + col_idx) % colors.len();
                let bg_i = (row_idx * 3 + col_idx * 7) % colors.len();
                cell.fg = colors[fg_i];
                cell.bg = colors[bg_i];
                cell.bold = (row_idx + col_idx) % 7 == 0;
                cell.italic = (row_idx * 2 + col_idx) % 11 == 0;
                cell.underline = (row_idx + col_idx * 3) % 13 == 0;
                cell.reverse = (row_idx * 5 + col_idx) % 17 == 0;
            }
        }
    }

    #[test]
    fn benchmark_to_html() {
        let sizes: &[(usize, usize, &str)] = &[
            (80, 24, "80x24 (small)"),
            (120, 40, "120x40 (medium)"),
            (200, 50, "200x50 (large)"),
        ];

        for &(cols, rows, label) in sizes {
            let mut buf = Buffer::new(cols, rows, 5000);
            populate_buffer(&mut buf);

            let iterations = 100;
            let start = std::time::Instant::now();
            let mut total_bytes = 0usize;
            for _ in 0..iterations {
                let html = VttyRenderer::to_html(&buf);
                total_bytes += html.len();
            }
            let elapsed = start.elapsed();
            let avg_us = elapsed.as_micros() as f64 / iterations as f64;
            let avg_kb = total_bytes as f64 / iterations as f64 / 1024.0;
            eprintln!(
                "  to_html({}) — {} iterations, total {:.1?}, avg {:.0} µs/frame, {:.1} KB/frame",
                label, iterations, elapsed, avg_us, avg_kb
            );
        }
    }

    #[test]
    fn benchmark_buffer_diff() {
        let sizes: &[(usize, usize, &str)] = &[
            (80, 24, "80x24 (small)"),
            (120, 40, "120x40 (medium)"),
            (200, 50, "200x50 (large)"),
        ];

        for &(cols, rows, label) in sizes {
            let mut buf_a = Buffer::new(cols, rows, 5000);
            populate_buffer(&mut buf_a);

            let mut buf_b = buf_a.clone();
            // Change ~5% of cells (realistic for interactive editing)
            let change_count = (cols * rows) / 20;
            for i in 0..change_count {
                let r = i / cols;
                let c = i % cols;
                buf_b.rows[r][c].ch = '█';
                buf_b.rows[r][c].fg = [255, 0, 0];
            }

            let iterations = 10000;
            let start = std::time::Instant::now();
            for _ in 0..iterations {
                let _diff = buf_b.diff(&buf_a);
            }
            let elapsed = start.elapsed();
            let avg_ns = elapsed.as_nanos() as f64 / iterations as f64;
            eprintln!(
                "  diff({}) — {} iterations, total {:.1?}, avg {:.0} ns/diff",
                label, iterations, elapsed, avg_ns
            );
        }
    }

    /// Benchmark: JSON serialization of BufferDiff for WebSocket transport.
    /// Measures the time and output size of serializing diff data into the
    /// JSON format used by the vtty_diff WS message (Level 3).
    #[test]
    fn benchmark_diff_json_serialization() {
        let sizes: &[(usize, usize, &str)] = &[
            (80, 24, "80x24 (small)"),
            (120, 40, "120x40 (medium)"),
            (200, 50, "200x50 (large)"),
        ];
        // Test different change rates
        let change_rates: &[(f64, &str)] = &[
            (0.01, "1% (typing)"),
            (0.05, "5% (interactive)"),
            (0.25, "25% (partial update)"),
            (0.50, "50% (half-screen)"),
        ];

        for &(cols, rows, label) in sizes {
            let mut buf_a = Buffer::new(cols, rows, 5000);
            populate_buffer(&mut buf_a);

            for &(rate, rate_label) in change_rates {
                let mut buf_b = buf_a.clone();
                let change_count = ((cols * rows) as f64 * rate) as usize;
                for i in 0..change_count {
                    let r = i / cols;
                    let c = i % cols;
                    buf_b.rows[r][c].ch = '█';
                    buf_b.rows[r][c].fg = [255, 0, 0];
                    buf_b.rows[r][c].bold = true;
                }

                let diff = buf_b.diff(&buf_a);

                let iterations = 100;
                let start = std::time::Instant::now();
                let mut total_bytes = 0usize;
                for _ in 0..iterations {
                    let json = serde_json::json!({
                        "type": "vtty_diff",
                        "data": {
                            "generation": 1u64,
                            "cursor": {"row": 10, "col": 42},
                            "dimensions": {"rows": rows, "cols": cols},
                            "cursor_visible": true,
                            "alternate_screen": false,
                            "changed_count": diff.changed_count,
                            "cells": diff.cells,
                        }
                    })
                    .to_string();
                    total_bytes += json.len();
                }
                let elapsed = start.elapsed();
                let avg_us = elapsed.as_micros() as f64 / iterations as f64;
                let avg_kb = total_bytes as f64 / iterations as f64 / 1024.0;
                eprintln!(
                    "  diff_json({}, {}) — {} iters, total {:.1?}, avg {:.0} µs/serialize, {:.1} KB/msg ({}/{} cells)",
                    label, rate_label, iterations, elapsed, avg_us, avg_kb, diff.changed_count, cols * rows
                );
            }
        }
    }
}
