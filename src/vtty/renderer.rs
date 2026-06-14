use super::buffer::Buffer;
use super::cell::Cell;

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
    // ────────────────────────────────────────────────────────────────
    // ANSI rendering
    // ────────────────────────────────────────────────────────────────

    /// Serialize the visible rows of the buffer to ANSI text with color codes.
    pub fn to_ansi(buffer: &Buffer) -> String {
        to_ansi_impl(buffer.rows.iter())
    }

    /// Serialize the full buffer (scrollback + visible rows) to ANSI text
    /// with color codes, SGR attribute resets, and line/screen clearing.
    pub fn to_ansi_full(buffer: &Buffer) -> String {
        to_ansi_impl(buffer.scrollback.iter().chain(buffer.rows.iter()))
    }

    // ────────────────────────────────────────────────────────────────
    // HTML rendering
    // ────────────────────────────────────────────────────────────────

    /// Serialize visible rows to HTML with run-length encoding.
    pub fn to_html(buffer: &Buffer) -> String {
        to_html_impl(buffer.rows.iter().map(|r| r.as_slice()), buffer.width)
    }

    /// Same format as to_html() but includes scrollback lines.
    pub fn to_html_scrollback(
        buffer: &Buffer,
        scrollback_offset: usize,
        visible_rows: usize,
    ) -> String {
        let total_lines = buffer.total_lines();
        let max_offset = total_lines.saturating_sub(visible_rows);
        let effective_offset = scrollback_offset.min(max_offset);

        let rows: Vec<&Vec<Cell>> = buffer
            .scrollback
            .iter()
            .chain(buffer.rows.iter())
            .skip(effective_offset)
            .take(visible_rows)
            .collect();

        to_html_impl(rows.iter().map(|r| r.as_slice()), buffer.width)
    }

    // ────────────────────────────────────────────────────────────────
    // Plain text rendering
    // ────────────────────────────────────────────────────────────────

    /// Serialize buffer to plain text (no formatting).
    pub fn to_plain(buffer: &Buffer) -> String {
        buffer
            .rows
            .iter()
            .map(|row| row.iter().filter(|c| c.width > 0).map(|c| c.ch).collect::<String>())
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
            .map(|row| row.iter().filter(|c| c.width > 0).map(|c| c.ch).collect::<String>())
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
            .map(|row| row.iter().filter(|c| c.width > 0).map(|c| c.ch).collect::<String>())
            .collect();
        all_lines
    }

    // ────────────────────────────────────────────────────────────────
    // PNG rendering (feature-gated)
    // ────────────────────────────────────────────────────────────────

    /// Render buffer as a PNG image.
    ///
    /// Wide characters (CJK, emoji) are rasterized at double cell width.
    /// The underlying monospace font may not contain glyphs for all Unicode
    /// code points (e.g. box-drawing symbols like ▽, emoji); fontdue
    /// substitutes a replacement glyph (tofu box) for missing glyphs.
    #[cfg(feature = "vrw")]
    pub fn to_png(buffer: &Buffer, font_size: f32, font_path: Option<&str>) -> anyhow::Result<Vec<u8>> {
        let font = match font_path {
            Some(path) => {
                let data = std::fs::read(path)?;
                fontdue::Font::from_bytes(data, fontdue::FontSettings::default())
                    .map_err(|e| anyhow::anyhow!("{}", e))?
            }
            None => {
                // Use IBM Plex Mono bundled as a static asset
                fontdue::Font::from_bytes(
                    include_bytes!("../../assets/IBM_Plex_Mono-Regular.ttf") as &[u8],
                    fontdue::FontSettings::default(),
                )
                .map_err(|e| anyhow::anyhow!("{}", e))?
            }
        };

        let cell_w = font_size * 0.6;
        let cell_h = font_size;
        let cols = buffer.width;
        let rows = buffer.rows.len();

        let img_w = (cols as f32 * cell_w).ceil() as u32;
        let img_h = (rows as f32 * cell_h).ceil() as u32;

        let mut img = image::ImageBuffer::new(img_w, img_h);
        // Fill background with dark terminal color
        let bg = image::Rgb([18, 18, 18]);
        for pixel in img.pixels_mut() {
            *pixel = bg;
        }

        for (row_idx, row) in buffer.rows.iter().enumerate().take(rows) {
            for (col_idx, cell) in row.iter().enumerate().take(cols) {
                // Skip wide-char continuation cells; they are covered by the
                // lead character's double-width rasterization.
                if cell.width == 0 {
                    continue;
                }
                let ch = if cell.is_empty() { ' ' } else { cell.ch };
                let fg = if cell.reverse { cell.bg } else { cell.fg };

                // Use double cell width for wide characters so their glyphs
                // are not clipped.  fontdue rasterizes at the requested size;
                // for a 2-column character we give it twice the horizontal
                // space so the advance width fits naturally.
                let raster_size = if cell.width == 2 {
                    font_size * 2.0
                } else {
                    font_size
                };

                // Rasterize glyph
                let (metrics, bitmap) = font.rasterize(ch, raster_size);

                let x0 = (col_idx as f32 * cell_w) as i32;
                let y0 = (row_idx as f32 * cell_h) as i32;

                for glyph_y in 0..metrics.height as i32 {
                    for glyph_x in 0..metrics.width as i32 {
                        let px = x0 + glyph_x + metrics.xmin;
                        let py = y0 + glyph_y + metrics.ymin;
                        if px < 0 || py < 0 || (px as u32) >= img_w || (py as u32) >= img_h {
                            continue;
                        }
                        let idx = (glyph_y * metrics.width as i32 + glyph_x) as usize;
                        let coverage = if idx < bitmap.len() { bitmap[idx] as f32 / 255.0 } else { 0.0 };
                        let alpha = coverage;
                        let inv_alpha = 1.0 - alpha;
                        let img_px = img.get_pixel_mut(px as u32, py as u32);
                        img_px[0] = (fg[0] as f32 * alpha + img_px[0] as f32 * inv_alpha) as u8;
                        img_px[1] = (fg[1] as f32 * alpha + img_px[1] as f32 * inv_alpha) as u8;
                        img_px[2] = (fg[2] as f32 * alpha + img_px[2] as f32 * inv_alpha) as u8;
                    }
                }
            }
        }

        let mut buf = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)?;
        Ok(buf)
    }
}

// ────────────────────────────────────────────────────────────────
// Internal ANSI rendering
// ────────────────────────────────────────────────────────────────

fn to_ansi_impl<'a>(rows: impl Iterator<Item = &'a Vec<Cell>>) -> String {
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

    for row in rows {
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

// ────────────────────────────────────────────────────────────────
// Internal HTML rendering
// ────────────────────────────────────────────────────────────────

fn to_html_impl<'a>(rows: impl Iterator<Item = &'a [Cell]>, width: usize) -> String {
    let row_count = rows.size_hint().0;
    let mut html = String::with_capacity(row_count * width * 20);
    let mut run_text: String = String::with_capacity(32);

    for row in rows {
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

            let run_width = cell.width;

            // Accumulate characters that share this exact style AND width.
            // Width-0 cells (wide-char continuations) use U+200B;
            // regular empty cells use a plain space.
            run_text.clear();
            let ch = if cell.width == 0 { '\u{200b}' } else if cell.is_empty() { ' ' } else { cell.ch };
            run_text.push(ch);
            let mut j = i + 1;
            while j < row_len {
                let next = &row[j];
                let nfg = if next.reverse { next.bg } else { next.fg };
                let nbg = if next.reverse { next.fg } else { next.bg };
                // Also break on width change so each span has a uniform
                // width class (w0/w1/w2) for correct column sizing.
                if nfg != fg || nbg != bg || next.bold != bold
                    || next.italic != italic || next.underline != underline
                    || next.strikethrough != strikethrough || next.blink != blink
                    || next.width != run_width
                {
                    break;
                }
                let nch = if next.width == 0 { '\u{200b}' } else if next.is_empty() { ' ' } else { next.ch };
                run_text.push(nch);
                j += 1;
            }

            // Width class: w0=continuation (zero-width), w1=normal, w2=wide.
            // The inline width (N*W ch) ensures characters respect the
            // terminal's column assignment regardless of the browser's
            // font metrics for ambiguous-width characters.
            let run_len = j - i;
            let (wclass, cell_ch) = match run_width {
                0 => ("c w0", 0),
                2 => ("c w2", 2),
                _ => ("c w1", 1),
            };

            // Build style string
            html.push_str("<span class=\"");
            html.push_str(wclass);
            html.push_str("\" style=\"width:");
            // Write width in ch units: run_len * cell_ch
            let total_ch = run_len * cell_ch;
            if total_ch > 0 {
                let mut buf = [0u8; 8];
                let mut pos = buf.len();
                let mut n = total_ch;
                while n > 0 {
                    pos -= 1;
                    buf[pos] = b'0' + (n % 10) as u8;
                    n /= 10;
                }
                html.push_str(std::str::from_utf8(&buf[pos..]).unwrap());
                html.push_str("ch");
            } else {
                html.push('0');
            }
            html.push_str(";color:");
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

#[cfg(test)]
mod tests {
    use super::super::cell::Cell;
    use super::*;

    #[test]
    fn test_to_html() {
        let mut buf = Buffer::new(5, 2, 100);
        buf.rows[0][0].ch = 'X';
        buf.rows[0][0].fg = [0, 255, 0];
        let html = VttyRenderer::to_html(&buf);
        // Uses hex color format (#00ff00) and CSS class "c" with width class
        assert!(html.contains("#00ff00"), "expected hex color for green fg");
        assert!(html.contains(r#"class="c w1""#), "expected CSS class 'c' with width class");
        assert!(html.contains("X"));
        // Should contain width in ch units in inline style
        assert!(html.contains("width:"), "expected width in inline style");
        assert!(html.contains("w1"), "expected w1 width class");
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
        // Should have at least 3 spans: A(span1), 你(span2), cont(span3)
        let span_count = html.matches(r#"<span class="c w"#).count();
        assert!(span_count >= 3, "wide char with different continuation style should produce separate spans, got {}", span_count);
        // Both 你 and the zero-width space should be present
        assert!(html.contains('你'));
        assert!(html.contains('\u{200b}'));
    }

    #[test]
    fn test_rle_empty_cells_use_space_not_zwsp() {
        // Normal empty cells (width=1, default attributes) should render as
        // regular spaces (U+0020), NOT as zero-width spaces (U+200B).
        let mut buf = Buffer::new(5, 1, 100);
        buf.rows[0][0].ch = 'A';
        buf.rows[0][1] = Cell::default(); // empty default cell
        buf.rows[0][2].ch = 'B';
        let html = VttyRenderer::to_html(&buf);
        assert!(!html.contains('\u{200b}'), "default empty cells should not produce zero-width spaces");
        assert!(html.contains('A'));
        assert!(html.contains('B'));
    }

    // ─── to_ansi_full includes scrollback ───

    #[test]
    fn test_to_ansi_full_includes_scrollback() {
        let mut buf = Buffer::new(5, 2, 100);
        buf.scrollback.push(vec![Cell::new('S')]);
        buf.rows[0][0].ch = 'V';
        let ansi = VttyRenderer::to_ansi_full(&buf);
        // Should contain both S (scrollback) and V (visible)
        assert!(ansi.contains('S'), "ANSI full should include scrollback content");
        assert!(ansi.contains('V'), "ANSI full should include visible content");
    }
}