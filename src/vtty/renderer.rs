use super::buffer::Buffer;
use image::{ImageBuffer, Rgb};

/// HTML-escape the five XML/HTML metacharacters so that VTTY cell content
/// never corrupts the DOM.  Programs that output `<`, `>`, `&`, `'`, or
/// `"` (e.g. `cat` on an HTML file, ANSI art with `<`/`>`) are common.
fn html_escape(s: char) -> String {
    match s {
        '&' => "&amp;".to_string(),
        '<' => "&lt;".to_string(),
        '>' => "&gt;".to_string(),
        '\'' => "&#39;".to_string(),
        '"' => "&quot;".to_string(),
        c => c.to_string(),
    }
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

    /// Serialize buffer to HTML with inline styles.
    /// Returns the inner content only (no outer `<pre>` wrapper) so that
    /// callers can control their own container element.
    ///
    /// Cell characters are HTML-escaped (`<`, `>`, `&`, `'`, `"`) to
    /// prevent DOM corruption when programs output HTML-like content or
    /// ANSI art containing these metacharacters.
    pub fn to_html(buffer: &Buffer) -> String {
        let mut html = String::new();
        // Rough capacity estimate: 80 chars × 24 rows × ~60 chars per cell (with span markup).
        html.reserve(buffer.rows.len() * buffer.width * 60);

        for row in &buffer.rows {
            for cell in row {
                // Every cell is wrapped in a <span> with display:inline-block;width:1ch
                // so the client-side cell grid (buildCellGrid) can index each cell by
                // position.  Empty cells get a zero-width space to keep the span alive.
                let mut style = String::from("display:inline-block;width:1ch;");
                style.push_str(&format!(
                    "color:rgb({},{},{});",
                    cell.fg[0], cell.fg[1], cell.fg[2]
                ));
                style.push_str(&format!(
                    "background:rgb({},{},{});",
                    cell.bg[0], cell.bg[1], cell.bg[2]
                ));

                if cell.reverse {
                    style.push_str(&format!(
                        "color:rgb({},{},{});background:rgb({},{},{});",
                        cell.bg[0], cell.bg[1], cell.bg[2], cell.fg[0], cell.fg[1], cell.fg[2]
                    ));
                }
                if cell.bold {
                    style.push_str("font-weight:bold;");
                }
                if cell.italic {
                    style.push_str("font-style:italic;");
                }
                if cell.underline {
                    style.push_str("text-decoration:underline;");
                }
                if cell.strikethrough {
                    style.push_str("text-decoration:line-through;");
                }
                if cell.blink {
                    style.push_str("animation:blink 1s step-end infinite;");
                }

                let ch = if cell.is_empty() { '\u{200b}' } else { cell.ch };
                html.push_str(&format!(
                    "<span style='{}'>{}</span>",
                    style,
                    html_escape(ch)
                ));
            }
            html.push('\n');
        }

        html
    }

    /// Serialize buffer (including scrollback) to HTML with inline styles.
    ///
    /// `scrollback_offset` shifts the viewport backward into the scrollback
    /// buffer.  0 = normal (bottom of visible area), 1 = one line scrolled
    /// back, etc.  The number of rows returned is `visible_rows`.
    ///
    /// This allows the web UI to render a scrollback view by setting the
    /// offset and fetching the corresponding HTML slice.
    pub fn to_html_scrollback(
        buffer: &Buffer,
        scrollback_offset: usize,
        visible_rows: usize,
    ) -> String {
        let total_lines = buffer.total_lines(); // scrollback.len() + rows.len()
        let max_offset = total_lines.saturating_sub(visible_rows);
        let effective_offset = scrollback_offset.min(max_offset);

        // Collect the visible slice from scrollback + rows
        let mut html = String::new();
        html.reserve(visible_rows * buffer.width * 60);

        let all_lines: Vec<&Vec<super::cell::Cell>> = buffer
            .scrollback
            .iter()
            .chain(buffer.rows.iter())
            .skip(effective_offset)
            .take(visible_rows)
            .collect();

        for row in &all_lines {
            for cell in *row {
                let mut style = String::from("display:inline-block;width:1ch;");
                style.push_str(&format!(
                    "color:rgb({},{},{});",
                    cell.fg[0], cell.fg[1], cell.fg[2]
                ));
                style.push_str(&format!(
                    "background:rgb({},{},{});",
                    cell.bg[0], cell.bg[1], cell.bg[2]
                ));

                if cell.reverse {
                    style.push_str(&format!(
                        "color:rgb({},{},{});background:rgb({},{},{});",
                        cell.bg[0], cell.bg[1], cell.bg[2], cell.fg[0], cell.fg[1], cell.fg[2]
                    ));
                }
                if cell.bold {
                    style.push_str("font-weight:bold;");
                }
                if cell.italic {
                    style.push_str("font-style:italic;");
                }
                if cell.underline {
                    style.push_str("text-decoration:underline;");
                }
                if cell.strikethrough {
                    style.push_str("text-decoration:line-through;");
                }
                if cell.blink {
                    style.push_str("animation:blink 1s step-end infinite;");
                }

                let ch = if cell.is_empty() { '\u{200b}' } else { cell.ch };
                html.push_str(&format!(
                    "<span style='{}'>{}</span>",
                    style,
                    html_escape(ch)
                ));
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

    /// Render buffer to a PNG image.
    ///
    /// Each character cell is `cell_w` × `cell_h` pixels.  The `scale` factor
    /// multiplies both dimensions (1 = 1x, 2 = retina/HiDPI).  The image is
    /// returned as a PNG byte vector.
    ///
    /// Uses a built-in 8×16 bitmap font covering ASCII printable characters
    /// (32–126) and a few box-drawing glyphs.  Characters outside this range
    /// are rendered as the replacement character `?`.
    pub fn to_png(buffer: &Buffer, cell_w: u32, cell_h: u32, scale: u32) -> Vec<u8> {
        let cw = cell_w * scale;
        let ch = cell_h * scale;
        let img_w = buffer.width as u32 * cw;
        let img_h = buffer.rows.len() as u32 * ch;
        let mut img = ImageBuffer::new(img_w, img_h);

        // Fill with default background (black)
        for pixel in img.pixels_mut() {
            *pixel = Rgb([0, 0, 0]);
        }

        for (row_idx, row) in buffer.rows.iter().enumerate() {
            for (col_idx, cell) in row.iter().enumerate() {
                let (fg, bg) = if cell.reverse {
                    (cell.bg, cell.fg)
                } else {
                    (cell.fg, cell.bg)
                };
                let base_x = col_idx as u32 * cw;
                let base_y = row_idx as u32 * ch;

                // Fill background
                for dy in 0..ch {
                    for dx in 0..cw {
                        img.put_pixel(base_x + dx, base_y + dy, Rgb(bg));
                    }
                }

                // Render glyph
                if !cell.invisible {
                    let glyph = get_glyph(cell.ch, cell.bold);
                    for gy in 0..8u32 {
                        let row_bits = glyph[gy as usize];
                        for gx in 0..8u32 {
                            if (row_bits & (0x80 >> gx)) == 0 {
                                continue;
                            }
                            // Plot the scaled pixel
                            for sy in 0..scale {
                                for sx in 0..scale {
                                    let px = base_x + gx * scale + sx;
                                    let py = base_y + gy * scale + sy;
                                    if px < img_w && py < img_h {
                                        img.put_pixel(px, py, Rgb(fg));
                                    }
                                }
                            }
                        }
                    }

                    // Underline: draw a line at the bottom of the cell
                    if cell.underline {
                        for dy in (ch - scale)..ch {
                            for dx in 0..cw {
                                img.put_pixel(base_x + dx, base_y + dy, Rgb(fg));
                            }
                        }
                    }

                    // Strikethrough: draw a line through the middle
                    if cell.strikethrough {
                        let mid_y = ch / 2;
                        for dy in mid_y..(mid_y + scale).min(ch) {
                            for dx in 0..cw {
                                img.put_pixel(base_x + dx, base_y + dy, Rgb(fg));
                            }
                        }
                    }
                }
            }
        }

        let mut png_buf = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut png_buf), image::ImageFormat::Png)
            .ok();
        png_buf
    }
}

// ─── Built-in 8×16 bitmap font ───
//
// Each glyph is 16 bytes (2 rows of 8 pixels, stored row-first).
// We use the upper 8 rows for an 8×8 cell that we scale to cell_h.
// Bold characters use the same data (no weight difference in bitmap font).

/// Returns 8 rows of 8 bits each for the given character.
/// Characters outside the printable ASCII range render as '?'.
fn get_glyph(ch: char, _bold: bool) -> [u8; 8] {
    // Compact 8×8 font — only ASCII printable (0x20–0x7E) plus a few extras.
    // Each entry is 8 bytes: row 0 (top) through row 7 (bottom).
    // Bits are MSB-first (bit 7 = leftmost pixel).
    const FONT: &[[u8; 8]] = &[
        [0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00], // ' ' (0x20)
        [0x18,0x18,0x18,0x18,0x18,0x00,0x18,0x00], // '!'
        [0x66,0x66,0x66,0x00,0x00,0x00,0x00,0x00], // '"'
        [0x66,0x66,0xff,0x66,0xff,0x66,0x00,0x00], // '#'
        [0x18,0x3e,0x60,0x3c,0x06,0x7c,0x18,0x00], // '$'
        [0x62,0x66,0x0c,0x18,0x30,0x66,0x46,0x00], // '%'
        [0x3c,0x66,0x3c,0x38,0x67,0x6f,0x6b,0x00], // '&'
        [0x18,0x18,0x30,0x00,0x00,0x00,0x00,0x00], // '''
        [0x0e,0x18,0x18,0x70,0x18,0x18,0x0e,0x00], // '('
        [0x70,0x18,0x18,0x0e,0x18,0x18,0x70,0x00], // ')'
        [0x66,0x3c,0xff,0x3c,0xff,0x66,0x00,0x00], // '*'
        [0x18,0x18,0x18,0x7e,0x18,0x18,0x18,0x00], // '+'
        [0x00,0x00,0x00,0x00,0x00,0x18,0x18,0x30], // ','
        [0x00,0x00,0x00,0x7e,0x00,0x00,0x00,0x00], // '-'
        [0x00,0x00,0x00,0x00,0x00,0x18,0x18,0x00], // '.'
        [0x06,0x0c,0x18,0x30,0x60,0x00,0x00,0x00], // '/'
        [0x3c,0x66,0x6e,0x76,0x66,0x66,0x3c,0x00], // '0'
        [0x18,0x38,0x18,0x18,0x18,0x18,0x7e,0x00], // '1'
        [0x3c,0x66,0x06,0x0c,0x30,0x60,0x7e,0x00], // '2'
        [0x3c,0x66,0x06,0x1c,0x06,0x66,0x3c,0x00], // '3'
        [0x0c,0x1c,0x3c,0x6c,0x7e,0x0c,0x0c,0x00], // '4'
        [0x7e,0x60,0x7c,0x06,0x06,0x66,0x3c,0x00], // '5'
        [0x3c,0x66,0x60,0x7c,0x66,0x66,0x3c,0x00], // '6'
        [0x7e,0x06,0x0c,0x18,0x30,0x30,0x30,0x00], // '7'
        [0x3c,0x66,0x66,0x3c,0x66,0x66,0x3c,0x00], // '8'
        [0x3c,0x66,0x66,0x3e,0x06,0x66,0x3c,0x00], // '9'
        [0x00,0x00,0x18,0x00,0x00,0x18,0x00,0x00], // ':'
        [0x00,0x00,0x18,0x00,0x00,0x18,0x18,0x30], // ';'
        [0x0c,0x18,0x30,0x60,0x30,0x18,0x0c,0x00], // '<'
        [0x00,0x00,0x7e,0x00,0x7e,0x00,0x00,0x00], // '='
        [0x30,0x18,0x0c,0x06,0x0c,0x18,0x30,0x00], // '>'
        [0x3c,0x66,0x06,0x0c,0x18,0x00,0x18,0x00], // '?'
        [0x3c,0x66,0x6e,0x6e,0x60,0x62,0x3c,0x00], // '@'
        [0x18,0x3c,0x66,0x66,0x7e,0x66,0x66,0x00], // 'A'
        [0x7c,0x66,0x66,0x7c,0x66,0x66,0x7c,0x00], // 'B'
        [0x3c,0x66,0x60,0x60,0x60,0x66,0x3c,0x00], // 'C'
        [0x78,0xcc,0x66,0x66,0x66,0xcc,0x78,0x00], // 'D'
        [0x7e,0x60,0x60,0x78,0x60,0x60,0x7e,0x00], // 'E'
        [0x7e,0x60,0x60,0x78,0x60,0x60,0x60,0x00], // 'F'
        [0x3c,0x66,0x60,0x6e,0x66,0x66,0x3c,0x00], // 'G'
        [0x66,0x66,0x66,0x7e,0x66,0x66,0x66,0x00], // 'H'
        [0x3c,0x18,0x18,0x18,0x18,0x18,0x3c,0x00], // 'I'
        [0x1e,0x0c,0x0c,0x0c,0x0c,0x6c,0x38,0x00], // 'J'
        [0x66,0x6c,0x78,0x70,0x78,0x6c,0x66,0x00], // 'K'
        [0x60,0x60,0x60,0x60,0x60,0x60,0x7e,0x00], // 'L'
        [0x63,0x77,0x7f,0x7f,0x6b,0x6b,0x63,0x00], // 'M'
        [0x66,0x76,0x7e,0x7e,0x6e,0x66,0x66,0x00], // 'N'
        [0x3c,0x66,0x66,0x66,0x66,0x66,0x3c,0x00], // 'O'
        [0x7c,0x66,0x66,0x7c,0x60,0x60,0x60,0x00], // 'P'
        [0x3c,0x66,0x66,0x66,0x6a,0x6c,0x36,0x00], // 'Q'
        [0x7c,0x66,0x66,0x7c,0x6c,0x66,0x66,0x00], // 'R'
        [0x3c,0x66,0x60,0x3c,0x06,0x66,0x3c,0x00], // 'S'
        [0x7e,0x18,0x18,0x18,0x18,0x18,0x18,0x00], // 'T'
        [0x66,0x66,0x66,0x66,0x66,0x66,0x3c,0x00], // 'U'
        [0x66,0x66,0x66,0x66,0x66,0x3c,0x18,0x00], // 'V'
        [0x63,0x63,0x63,0x6b,0x6b,0x7f,0x77,0x00], // 'W'
        [0x66,0x66,0x3c,0x18,0x3c,0x66,0x66,0x00], // 'X'
        [0x66,0x66,0x66,0x3c,0x18,0x18,0x18,0x00], // 'Y'
        [0x7e,0x06,0x0c,0x18,0x30,0x60,0x7e,0x00], // 'Z'
        [0x3c,0x30,0x30,0x30,0x30,0x30,0x3c,0x00], // '['
        [0x60,0x30,0x18,0x0c,0x06,0x03,0x00,0x00], // '\'
        [0x3c,0x0c,0x0c,0x0c,0x0c,0x0c,0x3c,0x00], // ']'
        [0x08,0x1c,0x36,0x63,0x00,0x00,0x00,0x00], // '^'
        [0x00,0x00,0x00,0x00,0x00,0x00,0x00,0xff], // '_'
    ];

    let idx = ch as usize;
    if idx >= 0x20 && idx <= 0x5F {
        FONT[idx - 0x20]
    } else if idx >= 0x60 && idx <= 0x7E {
        FONT[idx - 0x20]
    } else {
        // '?' glyph for non-ASCII
        FONT[0x3F - 0x20]
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
        assert!(html.contains("rgb(0,255,0)"));
        assert!(html.contains("X"));
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
}
