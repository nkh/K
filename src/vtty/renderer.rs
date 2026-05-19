use super::buffer::Buffer;

pub struct VttyRenderer;

impl VttyRenderer {
    pub fn to_ansi(buffer: &Buffer) -> String {
        // TODO: Serialize buffer back to ANSI text with escape sequences
        buffer.rows.iter()
            .map(|row| row.iter().map(|c| c.ch).collect::<String>())
            .collect::<Vec<_>>()
            .join("
")
    }

    pub fn to_html(buffer: &Buffer) -> String {
        // TODO: Serialize buffer to HTML with inline styles
        let mut html = String::from("<pre style='background:#000;color:#ccc;'>");
        for row in &buffer.rows {
            for cell in row {
                html.push(cell.ch);
            }
            html.push('
');
        }
        html.push_str("</pre>");
        html
    }
}
