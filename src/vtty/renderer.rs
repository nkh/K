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
        let mut html = String::with_capacity(total_cells * 20);
        let mut run_text: String = String::with_capacity(32);

        let width = buffer.width;
        for row in &buffer.rows {
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

                let run_width = cell.width;

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
                        || next.width != run_width
                    {
                        break;
                    }
                    let nch = if next.width == 0 { '\u{200b}' } else if next.is_empty() { ' ' } else { next.ch };
                    run_text.push(nch);
                    j += 1;
                }

                let run_len = j - i;
                let (wclass, cell_ch) = match run_width {
                    0 => ("c w0", 0),
                    2 => ("c w2", 2),
                    _ => ("c w1", 1),
                };

                html.push_str("<span class=\"");
                html.push_str(wclass);
                html.push_str("\" style=\"width:");
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
            .map(|row| row.iter().filter(|c| c.width > 0).map(|c| c.ch).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

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
        // Uses hex color format (#00ff00) and CSS class "c" with width class
        assert!(html.contains("#00ff00"), "expected hex color for green fg");
        assert!(html.contains("class=\"c "), "expected CSS class 'c' with width class");
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
        // Should have at least 4 spans: A(span1), 你(span2), cont(span3), B(merged with A or new)
        let span_count = html.matches("<span class=\"c ").count();
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
        // All same style, but width changes cause span breaks.
        // Expect: w1(A), w2(你), w0(zwsp), w1( B) = 4 spans
        assert!(html.contains('A'));
        assert!(html.contains('你'));
        assert!(html.contains('\u{200b}')); // continuation
        assert!(html.contains('B'));
        // Verify exactly one zero-width space (one wide char continuation)
        let zwsp_count = html.matches('\u{200b}').count();
        assert_eq!(zwsp_count, 1, "should have exactly 1 ZWSP for 1 wide char");
        // Verify width classes present
        assert!(html.contains("w1"), "should have w1 class for normal cells");
        assert!(html.contains("w2"), "should have w2 class for wide cells");
        assert!(html.contains("w0"), "should have w0 class for continuation cells");
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

    // ─── UTF-8 wide-char plain/ANSI tests ───

    #[test]
    fn test_to_plain_skips_continuation_cells() {
        // Wide char (width=2) + continuation (width=0) + trailing char.
        // to_plain must skip the continuation cell to preserve alignment.
        let mut buf = Buffer::new(5, 1, 100);
        buf.rows[0][1].ch = '你';
        buf.rows[0][1].width = 2;
        buf.rows[0][2].ch = ' '; // continuation placeholder
        buf.rows[0][2].width = 0;
        buf.rows[0][3].ch = 'X';
        let text = VttyRenderer::to_plain(&buf);
        // The continuation cell must NOT appear as a visible space
        assert!(!text.contains("  X"), "continuation cell should not produce visible space");
        assert!(text.contains("你"));
        assert!(text.contains("X"));
    }

    #[test]
    fn test_to_ansi_skips_continuation_cells() {
        // Same as above but for ANSI output.
        let mut buf = Buffer::new(5, 1, 100);
        buf.rows[0][1].ch = '你';
        buf.rows[0][1].width = 2;
        buf.rows[0][2].ch = ' '; // continuation
        buf.rows[0][2].width = 0;
        buf.rows[0][3].ch = 'Q';
        let ansi = VttyRenderer::to_ansi(&buf);
        // The continuation cell should not produce any output at all
        // Count non-escape characters to verify correct count
        let visible: String = ansi.chars().filter(|c| *c != '\x1b' && *c != '\n').collect();
        // Should have: ' '(col0) + '你'(col1) + 'Q'(col3) = 3 visible chars
        // (col2 continuation is skipped)
        assert!(visible.contains('你'), "wide char should be in ANSI output");
        assert!(visible.contains('Q'), "trailing char should be in ANSI output");
    }

    #[test]
    fn test_to_plain_multiple_wide_chars() {
        // Multiple wide chars on one row: "你 好" (two CJK chars with space between)
        // Columns: 0=你(w2), 1=cont(w0), 2=space(w1), 3=好(w2), 4=cont(w0)
        let mut buf = Buffer::new(5, 1, 100);
        buf.rows[0][0].ch = '你';
        buf.rows[0][0].width = 2;
        buf.rows[0][1].ch = ' ';
        buf.rows[0][1].width = 0;
        buf.rows[0][2].ch = ' ';
        buf.rows[0][2].width = 1;
        buf.rows[0][3].ch = '好';
        buf.rows[0][3].width = 2;
        buf.rows[0][4].ch = ' ';
        buf.rows[0][4].width = 0;
        let text = VttyRenderer::to_plain(&buf);
        // Should have: 你 + space + 好 (no continuation spaces)
        assert!(text.contains('你'));
        assert!(text.contains('好'));
        // Only one space between them (col 2)
        let chars: Vec<char> = text.chars().filter(|c| !c.is_whitespace() || *c == ' ').collect();
        let space_count = chars.iter().filter(|c| **c == ' ').count();
        assert_eq!(space_count, 1, "should have exactly 1 visible space between two wide chars");
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

    // ─── Unicode rendering tests ───

    #[test]
    fn test_html_renders_triangle_down() {
        // ▽ (U+25BD) must render as a literal UTF-8 character in the HTML.
        let mut buf = Buffer::new(5, 1, 100);
        buf.rows[0][2].ch = '\u{25bd}'; // ▽
        let html = VttyRenderer::to_html(&buf);
        assert!(html.contains('\u{25bd}'), "▽ must appear in HTML output");
        // Must NOT be HTML-escaped
        assert!(!html.contains("&#25bd;"), "▽ should not be numeric-escaped");
    }

    #[test]
    fn test_html_renders_box_drawing_chars() {
        // Box drawing characters are commonly used in terminal UIs.
        let mut buf = Buffer::new(4, 1, 100);
        buf.rows[0][0].ch = '┌';
        buf.rows[0][1].ch = '─';
        buf.rows[0][2].ch = '┐';
        let html = VttyRenderer::to_html(&buf);
        assert!(html.contains('┌'), "┌ must appear in HTML");
        assert!(html.contains('─'), "─ must appear in HTML");
        assert!(html.contains('┐'), "┐ must appear in HTML");
    }

    #[test]
    fn test_html_renders_geometric_shapes() {
        let mut buf = Buffer::new(10, 1, 100);
        buf.rows[0][0].ch = '\u{25bd}'; // ▽
        buf.rows[0][1].ch = '\u{25b3}'; // △
        buf.rows[0][2].ch = '\u{25c0}'; // ◀
        buf.rows[0][3].ch = '\u{25b6}'; // ▶
        buf.rows[0][4].ch = '\u{25c6}'; // ◆
        let html = VttyRenderer::to_html(&buf);
        for ch in ['\u{25bd}', '\u{25b3}', '\u{25c0}', '\u{25b6}', '\u{25c6}'] {
            assert!(html.contains(ch), "{:?} must appear in HTML", ch);
        }
    }

    #[test]
    fn test_html_renders_arrows_and_symbols() {
        let mut buf = Buffer::new(10, 1, 100);
        buf.rows[0][0].ch = '→';
        buf.rows[0][1].ch = '←';
        buf.rows[0][2].ch = '↑';
        buf.rows[0][3].ch = '↓';
        buf.rows[0][4].ch = '±';
        buf.rows[0][5].ch = '°';
        buf.rows[0][6].ch = '€';
        buf.rows[0][7].ch = 'µ';
        let html = VttyRenderer::to_html(&buf);
        for ch in ['→', '←', '↑', '↓', '±', '°', '€', 'µ'] {
            assert!(html.contains(ch), "{:?} must appear in HTML", ch);
        }
    }

    #[test]
    fn test_html_renders_emoji() {
        // Emoji (supplementary plane) should render as literal UTF-8 in HTML.
        let mut buf = Buffer::new(6, 1, 100);
        buf.rows[0][0].ch = '😊';
        buf.rows[0][0].width = 2;
        buf.rows[0][1].ch = ' '; // continuation
        buf.rows[0][1].width = 0;
        buf.rows[0][2].ch = '🔥';
        buf.rows[0][2].width = 2;
        buf.rows[0][3].ch = ' '; // continuation
        buf.rows[0][3].width = 0;
        let html = VttyRenderer::to_html(&buf);
        assert!(html.contains('😊'), "emoji must appear in HTML");
        assert!(html.contains('🔥'), "emoji must appear in HTML");
        // Should have exactly 2 zero-width spaces (2 wide char continuations)
        let zwsp_count = html.matches('\u{200b}').count();
        assert_eq!(zwsp_count, 2, "should have exactly 2 ZWSP for 2 emoji");
    }

    #[test]
    fn test_ansi_renders_unicode_chars() {
        // Unicode characters should pass through ANSI output unchanged.
        let mut buf = Buffer::new(10, 1, 100);
        buf.rows[0][0].ch = '\u{25bd}'; // ▽
        buf.rows[0][0].fg = [255, 0, 0];
        buf.rows[0][1].ch = '你';
        buf.rows[0][1].width = 2;
        buf.rows[0][2].ch = ' ';
        buf.rows[0][2].width = 0;
        let ansi = VttyRenderer::to_ansi(&buf);
        assert!(ansi.contains('\u{25bd}'), "▽ must appear in ANSI output");
        assert!(ansi.contains('你'), "CJK char must appear in ANSI output");
    }

    #[test]
    fn test_plain_renders_unicode_chars() {
        // Unicode characters should pass through plain text output.
        let mut buf = Buffer::new(10, 1, 100);
        buf.rows[0][0].ch = '\u{25bd}'; // ▽
        buf.rows[0][1].ch = '你';
        buf.rows[0][1].width = 2;
        buf.rows[0][2].ch = ' ';
        buf.rows[0][2].width = 0;
        buf.rows[0][3].ch = '€';
        let plain = VttyRenderer::to_plain(&buf);
        assert!(plain.contains('\u{25bd}'), "▽ must appear in plain output");
        assert!(plain.contains('你'), "CJK char must appear in plain output");
        assert!(plain.contains('€'), "€ must appear in plain output");
    }

    // ─── End-to-end UTF8 column alignment tests (emulator → HTML) ───

    /// Strip HTML tags and decode standard entities to get plain text content.
    fn html_to_text(html: &str) -> String {
        let decoded = html
            .replace("&amp;", "&")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&#39;", "'")
            .replace("&quot;", "\"");
        let mut result = String::new();
        let mut in_tag = false;
        for ch in decoded.chars() {
            match ch {
                '<' => in_tag = true,
                '>' => in_tag = false,
                _ if !in_tag => result.push(ch),
                _ => {}
            }
        }
        result
    }

    /// Count visual columns for a text string, as a monospace <pre> would.
    /// \u200b (zero-width space) = 0; everything else uses unicode-width.
    fn visual_columns(text: &str) -> usize {
        use super::super::cell::char_width;
        text.chars()
            .map(|c| if c == '\u{200b}' { 0 } else { char_width(c) as usize })
            .sum()
    }

    #[test]
    fn test_utf8_column_alignment_emulator_to_html() {
        // Feed various UTF8 strings through the emulator, then verify:
        //  1. Buffer rows have correct cell width sums (= buffer width)
        //  2. HTML visual column count per row = buffer width
        //  3. Plain text visual column count per row = buffer width
        //  4. Each test string's characters appear in the HTML
        use super::super::emulator::VttyEmulator;

        let test_strings: &[&str] = &[
            // Geometric symbols (ambiguous width → 1)
            "▽△◀▶◆●★✓✗",
            // Box drawing (ambiguous width → 1)
            "┌──┐│└──┘├┤┬┴┼",
            // CJK (width=2 each)
            "你好世界",
            // Mixed ASCII + geometric symbols
            "Hello ▽ World △ End",
            // Mixed ASCII + CJK
            "Test 你好 World",
            // Emoji (width=2 each) interspersed with ASCII
            "A😊B🔥C🚀D",
            // Arrows and math/special symbols
            "→←↑↓↔±×÷°²³µ",
            // Currency symbols
            "€£¥¢©®™",
            // Precomposed accented Latin (all width=1)
            "éüñøæß",
            // Long mixed line with many character types
            "A▽B你C🔥D€F┌G好H✓I▶J●K→L",
            // Multiple geometric symbols with spaces
            "▽ △ ◀ ▶ ◆ ● ★ ✓ ✗ ▽ △",
            // Tab + UTF8 (tab stops at 8-column boundaries)
            "\t▽\t△",
            // Multiple CJK chars packed together
            "中文测试日本語テスト",
            // Fill almost entire line to test right-margin behavior
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA▽",
        ];

        let cols: usize = 80;
        let rows = test_strings.len() + 2; // extra rows for margin

        let mut emu = VttyEmulator::new(rows as u16, cols as u16, 1000);

        for s in test_strings {
            emu.feed_str(s);
            emu.feed_str("\n");
        }

        let buf = emu.snapshot();

        // 1. Buffer column alignment: sum of cell widths = buffer width per row
        for (row_idx, row) in buf.rows.iter().enumerate() {
            let total_width: usize = row.iter().map(|c| c.width as usize).sum();
            assert_eq!(
                total_width, buf.width,
                "Row {}: cell width sum ({}) != buffer width ({})",
                row_idx, total_width, buf.width
            );
        }

        // 2. HTML visual column alignment per row
        let html = VttyRenderer::to_html(&buf);
        let html_lines: Vec<&str> = html.split('\n').collect();

        for (row_idx, line) in html_lines.iter().enumerate() {
            if row_idx >= buf.rows.len() { break; }
            if row_idx == html_lines.len() - 1 && line.is_empty() { continue; }

            let text = html_to_text(line);
            let vc = visual_columns(&text);
            assert_eq!(
                vc, buf.width,
                "Row {}: HTML visual cols ({}) != buffer width ({})\n  text: {:?}",
                row_idx, vc, buf.width, text
            );
        }

        // 3. Plain text visual column alignment per row
        let plain = VttyRenderer::to_plain(&buf);
        let plain_lines: Vec<&str> = plain.split('\n').collect();

        for (row_idx, line) in plain_lines.iter().enumerate() {
            if row_idx >= buf.rows.len() { break; }
            let vc = visual_columns(line);
            assert_eq!(
                vc, buf.width,
                "Row {}: plain visual cols ({}) != buffer width ({})\n  text: {:?}",
                row_idx, vc, buf.width, line
            );
        }

        // 4. Verify each test string's printable characters appear in the HTML
        for (i, s) in test_strings.iter().enumerate() {
            if i >= html_lines.len() { continue; }
            let text = html_to_text(html_lines[i]);
            for ch in s.chars() {
                // Control characters (tab, etc.) are processed by the emulator
                // into spaces/tab-stops and don't appear literally.
                if ch.is_control() { continue; }
                assert!(
                    text.contains(ch),
                    "Row {}: expected char {:?} (U+{:04X}) not found in HTML text: {:?}",
                    i, ch, ch as u32, text
                );
            }
        }
    }

    #[test]
    fn test_diff_with_unicode_chars() {
        // Unicode characters must survive buffer diff computation and
        // JSON serialization (as sent to the web client).
        let a = Buffer::new(5, 1, 100);
        let mut b_buf = Buffer::new(5, 1, 100);
        b_buf.rows[0][0].ch = '\u{25bd}'; // ▽
        b_buf.rows[0][1].ch = '你';
        b_buf.rows[0][1].width = 2;
        b_buf.rows[0][2].ch = ' ';
        b_buf.rows[0][2].width = 0;
        b_buf.rows[0][3].ch = '€';
        let diff = b_buf.diff(&a);
        // 4 cells changed (indices 0-3); cell 4 is untouched and matches default.
        assert_eq!(diff.changed_count, 4);
        // Verify the specific characters are in the diff
        let has_triangle = diff.cells.iter().any(|c| c.ch == '\u{25bd}');
        assert!(has_triangle, "▽ must be in diff");
        let has_cjk = diff.cells.iter().any(|c| c.ch == '你');
        assert!(has_cjk, "你 must be in diff");
        let has_euro = diff.cells.iter().any(|c| c.ch == '€');
        assert!(has_euro, "€ must be in diff");
        // JSON round-trip must preserve Unicode characters
        let json = serde_json::to_string(&diff).unwrap();
        assert!(json.contains('\u{25bd}'), "▽ must survive JSON serialization");
        assert!(json.contains("你"), "你 must survive JSON serialization");
        assert!(json.contains('€'), "€ must survive JSON serialization");
    }

    #[test]
    fn test_html_mixed_unicode_row() {
        // A realistic row: "─ ▽ 你 €"
        // Columns: 0=─(w1), 1=space(w1), 2=▽(w1), 3=space(w1), 4-5=你(w2+w0)
        // Use a wider buffer
        let mut buf = Buffer::new(10, 1, 100);
        buf.rows[0][0].ch = '─';
        buf.rows[0][0].fg = [200, 200, 200];
        buf.rows[0][2].ch = '\u{25bd}'; // ▽
        buf.rows[0][2].fg = [200, 200, 200];
        buf.rows[0][4].ch = '你';
        buf.rows[0][4].fg = [200, 200, 200];
        buf.rows[0][4].width = 2;
        buf.rows[0][5].ch = ' ';
        buf.rows[0][5].width = 0;
        buf.rows[0][5].fg = [200, 200, 200];
        buf.rows[0][6].ch = '€';
        buf.rows[0][6].fg = [200, 200, 200];
        let html = VttyRenderer::to_html(&buf);
        // All characters must be present
        assert!(html.contains('─'));
        assert!(html.contains('\u{25bd}'));
        assert!(html.contains('你'));
        assert!(html.contains('€'));
        // Must have exactly 1 ZWSP (for the CJK continuation)
        let zwsp_count = html.matches('\u{200b}').count();
        assert_eq!(zwsp_count, 1);
    }

    #[test]
    fn test_html_unicode_with_different_styles() {
        // Unicode chars with different styles should not be merged.
        let mut buf = Buffer::new(3, 1, 100);
        buf.rows[0][0].ch = '\u{25bd}'; // ▽ red
        buf.rows[0][0].fg = [255, 0, 0];
        buf.rows[0][1].ch = '\u{25bd}'; // ▽ blue
        buf.rows[0][1].fg = [0, 0, 255];
        buf.rows[0][2].ch = '\u{25bd}'; // ▽ green
        buf.rows[0][2].fg = [0, 255, 0];
        let html = VttyRenderer::to_html(&buf);
        // Three different styles → three separate spans
        let span_count = html.matches("<span class=\"c ").count();
        assert_eq!(span_count, 3, "3 different-colored ▽ should produce 3 spans");
    }
}
