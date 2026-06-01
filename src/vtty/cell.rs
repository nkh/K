use serde::{Deserialize, Serialize};

/// A single cell in the terminal buffer.
/// Each cell stores a Unicode character and its visual attributes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cell {
    pub ch: char,
    pub fg: [u8; 3],
    pub bg: [u8; 3],
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub blink: bool,
    pub reverse: bool,
    pub invisible: bool,
    pub strikethrough: bool,
    /// Display width of the character: 0 (continuation of wide char), 1, or 2.
    #[serde(default = "default_width")]
    pub width: u8,
}

fn default_width() -> u8 {
    1
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            ch: ' ',
            fg: [204, 204, 204],
            bg: [0, 0, 0],
            bold: false,
            italic: false,
            underline: false,
            blink: false,
            reverse: false,
            invisible: false,
            strikethrough: false,
            width: 1,
        }
    }
}

impl Cell {
    pub fn new(ch: char) -> Self {
        Self {
            ch,
            ..Default::default()
        }
    }

    pub fn with_colors(ch: char, fg: [u8; 3], bg: [u8; 3]) -> Self {
        Self {
            ch,
            fg,
            bg,
            ..Default::default()
        }
    }

    pub fn reset_attrs(&mut self) {
        self.fg = [204, 204, 204];
        self.bg = [0, 0, 0];
        self.bold = false;
        self.italic = false;
        self.underline = false;
        self.blink = false;
        self.reverse = false;
        self.invisible = false;
        self.strikethrough = false;
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Whether this cell is a wide-character continuation (width=0).
    pub fn is_wide_continuation(&self) -> bool {
        self.width == 0
    }

    pub fn is_empty(&self) -> bool {
        self.ch == ' '
            && self.fg == [204, 204, 204]
            && self.bg == [0, 0, 0]
            && !self.bold
            && !self.italic
            && !self.underline
            && !self.blink
            && !self.reverse
            && !self.invisible
            && !self.strikethrough
    }
}

/// Compute the terminal display width of a Unicode character.
/// Returns 2 for East Asian Wide / Fullwidth characters (CJK, emoji, etc.),
/// 1 for all other characters.
pub fn char_width(ch: char) -> u8 {
    use unicode_width::UnicodeWidthChar;
    ch.width().unwrap_or(1) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cell_default() {
        let c = Cell::default();
        assert_eq!(c.ch, ' ');
        assert_eq!(c.fg, [204, 204, 204]);
        assert!(!c.bold);
    }

    #[test]
    fn test_cell_new() {
        let c = Cell::new('X');
        assert_eq!(c.ch, 'X');
        assert_eq!(c.fg, [204, 204, 204]);
    }

    #[test]
    fn test_cell_clear() {
        let mut c = Cell::with_colors('A', [255, 0, 0], [0, 0, 0]);
        c.bold = true;
        c.clear();
        assert_eq!(c.ch, ' ');
        assert!(!c.bold);
    }

    #[test]
    fn test_cell_is_empty() {
        assert!(Cell::default().is_empty());
        let mut c = Cell::default();
        c.ch = 'A';
        assert!(!c.is_empty());
    }

    // ─── Unicode char_width tests ───

    #[test]
    fn test_char_width_ascii() {
        assert_eq!(char_width('A'), 1);
        assert_eq!(char_width(' '), 1);
        assert_eq!(char_width('~'), 1);
    }

    #[test]
    fn test_char_width_geometric_symbols() {
        // ▽ U+25BD BLACK DOWN-POINTING TRIANGLE (ambiguous width → 1)
        assert_eq!(char_width('\u{25bd}'), 1, "▽ should be width 1");
        // △ U+25B3 BLACK UP-POINTING TRIANGLE
        assert_eq!(char_width('\u{25b3}'), 1);
        // ◀ U+25C0 BLACK LEFT-POINTING TRIANGLE
        assert_eq!(char_width('\u{25c0}'), 1);
        // ▶ U+25B6 BLACK RIGHT-POINTING TRIANGLE
        assert_eq!(char_width('\u{25b6}'), 1);
        // ◆ U+25C6 BLACK DIAMOND
        assert_eq!(char_width('\u{25c6}'), 1);
        // ● U+25CF BLACK CIRCLE
        assert_eq!(char_width('\u{25cf}'), 1);
        // ★ U+2605 BLACK STAR
        assert_eq!(char_width('\u{2605}'), 1);
        // ✓ U+2713 CHECK MARK
        assert_eq!(char_width('\u{2713}'), 1);
        // ✗ U+2717 BALLOT X
        assert_eq!(char_width('\u{2717}'), 1);
    }

    #[test]
    fn test_char_width_box_drawing() {
        // Box drawing characters are all single-width (ambiguous → 1)
        assert_eq!(char_width('─'), 1); // U+2500
        assert_eq!(char_width('│'), 1); // U+2502
        assert_eq!(char_width('┌'), 1); // U+250C
        assert_eq!(char_width('┐'), 1); // U+2510
        assert_eq!(char_width('└'), 1); // U+2514
        assert_eq!(char_width('┘'), 1); // U+2518
        assert_eq!(char_width('├'), 1); // U+251C
        assert_eq!(char_width('┤'), 1); // U+2524
        assert_eq!(char_width('┬'), 1); // U+252C
        assert_eq!(char_width('┴'), 1); // U+2534
        assert_eq!(char_width('┼'), 1); // U+253C
    }

    #[test]
    fn test_char_width_cjk() {
        // CJK unified ideographs are width 2
        assert_eq!(char_width('你'), 2);
        assert_eq!(char_width('好'), 2);
        assert_eq!(char_width('中'), 2);
        assert_eq!(char_width('日'), 2);
        // Fullwidth forms are also width 2
        assert_eq!(char_width('\u{ff01}'), 2); // FULLWIDTH EXCLAMATION MARK
        assert_eq!(char_width('\u{ff21}'), 2); // FULLWIDTH LATIN CAPITAL LETTER A
    }

    #[test]
    fn test_char_width_emoji() {
        // Emoji in the supplementary plane are width 2
        assert_eq!(char_width('😊'), 2); // U+1F60A
        assert_eq!(char_width('🔥'), 2); // U+1F525
        assert_eq!(char_width('🚀'), 2); // U+1F680
        assert_eq!(char_width('✅'), 2); // U+2705 (BMP emoji)
    }

    #[test]
    fn test_char_width_combining_marks() {
        // Combining marks have width 0 (they overlay the preceding character)
        assert_eq!(char_width('\u{0301}'), 0); // COMBINING ACUTE ACCENT
        assert_eq!(char_width('\u{0308}'), 0); // COMBINING DIAERESIS
        assert_eq!(char_width('\u{0300}'), 0); // COMBINING GRAVE ACCENT
        assert_eq!(char_width('\u{0327}'), 0); // COMBINING CEDILLA
    }

    #[test]
    fn test_char_width_control_chars() {
        // Control characters return None from unicode-width, we default to 1
        assert_eq!(char_width('\u{0000}'), 1); // null
        assert_eq!(char_width('\u{0007}'), 1); // bell
        assert_eq!(char_width('\u{000a}'), 1); // line feed
        assert_eq!(char_width('\u{001b}'), 1); // escape
    }

    #[test]
    fn test_char_width_misc_symbols() {
        // Various symbols that should be width 1
        assert_eq!(char_width('→'), 1); // U+2192 RIGHTWARDS ARROW
        assert_eq!(char_width('←'), 1); // U+2190 LEFTWARDS ARROW
        assert_eq!(char_width('↑'), 1); // U+2191 UPWARDS ARROW
        assert_eq!(char_width('↓'), 1); // U+2193 DOWNWARDS ARROW
        assert_eq!(char_width('↔'), 1); // U+2194 LEFT RIGHT ARROW
        assert_eq!(char_width('±'), 1); // U+00B1 PLUS-MINUS SIGN
        assert_eq!(char_width('×'), 1); // U+00D7 MULTIPLICATION SIGN
        assert_eq!(char_width('÷'), 1); // U+00F7 DIVISION SIGN
        assert_eq!(char_width('°'), 1); // U+00B0 DEGREE SIGN
        assert_eq!(char_width('²'), 1); // U+00B2 SUPERSCRIPT TWO
        assert_eq!(char_width('³'), 1); // U+00B3 SUPERSCRIPT THREE
        assert_eq!(char_width('µ'), 1); // U+00B5 MICRO SIGN
        assert_eq!(char_width('©'), 1); // U+00A9 COPYRIGHT SIGN
        assert_eq!(char_width('®'), 1); // U+00AE REGISTERED SIGN
        assert_eq!(char_width('™'), 1); // U+2122 TRADE MARK SIGN
        assert_eq!(char_width('€'), 1); // U+20AC EURO SIGN
        assert_eq!(char_width('£'), 1); // U+00A3 POUND SIGN
        assert_eq!(char_width('¥'), 1); // U+00A5 YEN SIGN
        assert_eq!(char_width('¢'), 1); // U+00A2 CENT SIGN
    }

    #[test]
    fn test_char_width_accented() {
        // Precomposed accented Latin characters (width 1)
        assert_eq!(char_width('é'), 1);
        assert_eq!(char_width('ü'), 1);
        assert_eq!(char_width('ñ'), 1);
        assert_eq!(char_width('ø'), 1);
        assert_eq!(char_width('æ'), 1);
        assert_eq!(char_width('ß'), 1);
    }
}
