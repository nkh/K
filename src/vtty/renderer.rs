use super::buffer::Buffer;

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
    /// Returns the inner content only (no outer <pre> wrapper) so that
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
                style.push_str(&format!("color:rgb({},{},{});", cell.fg[0], cell.fg[1], cell.fg[2]));
                style.push_str(&format!("background:rgb({},{},{});", cell.bg[0], cell.bg[1], cell.bg[2]));

                if cell.reverse {
                    style.push_str(&format!("color:rgb({},{},{});background:rgb({},{},{});",
                        cell.bg[0], cell.bg[1], cell.bg[2],
                        cell.fg[0], cell.fg[1], cell.fg[2]));
                }
                if cell.bold { style.push_str("font-weight:bold;"); }
                if cell.italic { style.push_str("font-style:italic;"); }
                if cell.underline { style.push_str("text-decoration:underline;"); }
                if cell.strikethrough { style.push_str("text-decoration:line-through;"); }
                if cell.blink { style.push_str("animation:blink 1s step-end infinite;"); }

                let ch = if cell.is_empty() { '\u{200b}' } else { cell.ch };
                html.push_str(&format!("<span style='{}'>{}</span>", style, html_escape(ch)));
            }
            html.push('\n');
        }

        html
    }

    /// Serialize buffer to plain text (no formatting).
    pub fn to_plain(buffer: &Buffer) -> String {
        buffer.rows.iter()
            .map(|row| row.iter().map(|c| c.ch).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Get a range of lines as plain text.
    pub fn lines_plain(buffer: &Buffer, start: usize, count: usize) -> Vec<String> {
        buffer.rows.iter()
            .skip(start)
            .take(count)
            .map(|row| row.iter().map(|c| c.ch).collect::<String>())
            .collect()
    }

    /// Get a range of lines including scrollback.
    pub fn lines_with_scrollback(buffer: &Buffer, start: usize, count: usize) -> Vec<String> {
        let all_lines: Vec<_> = buffer.scrollback.iter()
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
    use super::*;
    use super::super::cell::Cell;

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
        assert_eq!(amp_count, 5, "expected exactly 5 & chars (one per escaped entity), got {}", amp_count);
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
