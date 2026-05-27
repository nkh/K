/// ANSI/VT100 escape sequence parser.
/// Streaming byte parser implemented as a state machine.
///
/// Maximum number of raw bytes allowed in OSC/DCS string content.
/// Prevents unbounded memory growth from unterminated sequences.
const MAX_STRING_LENGTH: usize = 8192;

#[derive(Debug, Clone, PartialEq)]
pub enum AnsiToken {
    Text(String),
    Control(u8),
    Csi { params: Vec<Vec<u16>>, intermediate: Vec<u8>, final_byte: u8 },
    Osc(String),
    Escape(u8),
    /// ESC followed by one or more intermediate bytes (0x20-0x2f) and a
    /// final byte (0x30-0x7e).  These are independent escape sequences —
    /// NOT CSI sequences.  Examples:
    ///   ESC ( B  — designate G0 charset as ASCII
    ///   ESC ) 0  — designate G1 charset as line drawing
    ///   ESC # 3  — DECDHL (double-height line, top half)
    ///   ESC SP F — S7C1T (7-bit C1 mode)
    EscSequence { intermediate: Vec<u8>, final_byte: u8 },
    Dcs { params: Vec<Vec<u16>>, intermediate: Vec<u8>, final_byte: u8, data: String },
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ParseState {
    Ground,
    Escape,
    CsiParam,
    CsiIntermediate,
    OscString,
    DcsEntry,
    DcsParam,
    DcsIntermediate,
    DcsString,
    String,
    /// ESC followed by intermediate bytes (0x20-0x2f), awaiting final byte.
    EscapeIntermediate,
}

pub struct AnsiParser {
    state: ParseState,
    buffer: Vec<u8>,
    csi_params: Vec<Vec<u16>>,
    current_param: Vec<u16>,
    intermediate: Vec<u8>,
    /// Raw byte buffer for accumulating OSC/DCS string content.
    /// Decoded to String only when the sequence is terminated, so that
    /// multi-byte UTF-8 inside strings is handled correctly.
    string_content: Vec<u8>,
    /// Raw byte buffer for accumulating UTF-8 text in Ground state.
    /// Decoded to a String only when a non-text token forces a flush.
    text_bytes: Vec<u8>,
    /// Stores the final byte of a DCS sequence.  Set when transitioning
    /// from DcsEntry/DcsParam/DcsIntermediate into DcsString, then used
    /// when the DCS is emitted on ST.
    dcs_final_byte: u8,
}

impl Default for AnsiParser {
    fn default() -> Self { Self::new() }
}

impl AnsiParser {
    pub fn new() -> Self {
        Self {
            state: ParseState::Ground,
            buffer: Vec::new(),
            csi_params: Vec::new(),
            current_param: Vec::new(),
            intermediate: Vec::new(),
            string_content: Vec::new(),
            text_bytes: Vec::new(),
            dcs_final_byte: 0,
        }
    }

    /// Flush the raw text byte buffer as a Text token if non-empty.
    /// Uses lossy conversion — any incomplete or invalid UTF-8 sequences
    /// are replaced with U+FFFD.  This is used when a non-text token
    /// (ESC, control byte) terminates the text stream, meaning any
    /// trailing incomplete bytes should be discarded as garbled.
    fn flush_text_bytes(&mut self, tokens: &mut Vec<AnsiToken>) {
        if self.text_bytes.is_empty() {
            return;
        }
        let text = String::from_utf8_lossy(&self.text_bytes).into_owned();
        tokens.push(AnsiToken::Text(text));
        self.text_bytes.clear();
    }

    /// Flush only the complete UTF-8 prefix of the text byte buffer.
    /// Any trailing incomplete UTF-8 bytes are kept in the buffer for
    /// the next parse() call.  This is used at the end of parse() to
    /// handle the common case where a chunk of input splits a multi-byte
    /// UTF-8 sequence.
    fn flush_text_bytes_preserve_incomplete(&mut self, tokens: &mut Vec<AnsiToken>) {
        if self.text_bytes.is_empty() {
            return;
        }
        match std::str::from_utf8(&self.text_bytes) {
            Ok(s) => {
                if !s.is_empty() {
                    tokens.push(AnsiToken::Text(s.to_owned()));
                }
                self.text_bytes.clear();
            }
            Err(e) => {
                let valid_up_to = e.valid_up_to();
                if valid_up_to > 0 {
                    // SAFETY: from_utf8 guarantees the first `valid_up_to` bytes are valid UTF-8.
                    let valid = unsafe {
                        String::from_utf8_unchecked(self.text_bytes[..valid_up_to].to_vec())
                    };
                    tokens.push(AnsiToken::Text(valid));
                }
                // Keep the trailing incomplete bytes for the next parse() call.
                // error_len() == 0 means the input was truncated (incomplete seq).
                if e.error_len().is_some() {
                    // The byte at valid_up_to is an invalid start byte, not just truncation.
                    // Replace it with U+FFFD and skip it.
                    self.text_bytes = self.text_bytes[valid_up_to + e.error_len().unwrap()..].to_vec();
                } else {
                    self.text_bytes = self.text_bytes[valid_up_to..].to_vec();
                }
            }
        }
    }

    /// Decode the accumulated raw bytes in `string_content` as UTF-8.
    fn decode_string_content(&self) -> String {
        String::from_utf8_lossy(&self.string_content).into_owned()
    }

    /// Push a raw byte into `string_content`, respecting the length limit.
    /// Returns false if the limit has been reached (caller should discard).
    fn push_string_byte(&mut self, byte: u8) {
        if self.string_content.len() < MAX_STRING_LENGTH {
            self.string_content.push(byte);
        }
    }

    /// Process a single byte in Ground state.  Extracted so that abort
    /// paths from other states can reprocess a byte from Ground without
    /// duplicating logic.
    fn process_ground_byte(&mut self, byte: u8, tokens: &mut Vec<AnsiToken>) {
        match byte {
            0x1b => {
                self.flush_text_bytes(tokens);
                self.state = ParseState::Escape;
                self.buffer.clear();
                self.buffer.push(byte);
            }
            0x00..=0x08 | 0x0b..=0x0c | 0x0e..=0x1a | 0x1c..=0x1f => {
                self.flush_text_bytes(tokens);
                tokens.push(AnsiToken::Control(byte));
            }
            0x7f => {}
            _ => { self.text_bytes.push(byte); }
        }
    }

    // ── Shared parameter parsing ──

    /// Parse a CSI/DCS parameter digit, sub-parameter separator, or
    /// parameter separator.  Returns true if the byte was consumed
    /// as a parameter byte (digit, ';', or ':').
    fn parse_param_digit_or_separator(&mut self, byte: u8) -> bool {
        match byte {
            0x30..=0x39 => {
                if let Some(last) = self.current_param.last_mut() {
                    *last = (*last * 10) + ((byte - b'0') as u16);
                } else {
                    self.current_param.push((byte - b'0') as u16);
                }
                true
            }
            b';' => {
                if self.current_param.is_empty() {
                    self.csi_params.push(vec![0]);
                } else {
                    self.csi_params.push(self.current_param.clone());
                    self.current_param.clear();
                }
                true
            }
            b':' => {
                self.current_param.push(0);
                true
            }
            _ => false,
        }
    }

    // ── State handlers (extracted from parse()) ──

    /// Handle a byte in the Escape state (after receiving ESC).
    /// Returns false if the byte should be reprocessed from Ground.
    fn handle_escape_byte(&mut self, byte: u8, tokens: &mut Vec<AnsiToken>) -> bool {
        self.buffer.push(byte);
        match byte {
            b'[' => {
                self.state = ParseState::CsiParam;
                self.csi_params.clear();
                self.current_param.clear();
                self.intermediate.clear();
            }
            b']' => {
                self.state = ParseState::OscString;
                self.string_content.clear();
            }
            b'P' => {
                self.state = ParseState::DcsEntry;
                self.csi_params.clear();
                self.current_param.clear();
                self.intermediate.clear();
                self.string_content.clear();
                self.dcs_final_byte = 0;
            }
            b'X' | b'^' | b'_' => {
                self.state = ParseState::String;
            }
            0x20..=0x2f => {
                // Intermediate byte after ESC — this begins
                // an independent escape sequence (charset
                // designation, DECDHL, etc.), NOT a CSI.
                self.intermediate.clear();
                self.intermediate.push(byte);
                self.state = ParseState::EscapeIntermediate;
            }
            0x30..=0x7e => {
                tokens.push(AnsiToken::Escape(byte));
                self.reset_to_ground();
            }
            _ => {
                // Unknown byte after ESC — abort the escape.
                // The ESC is silently dropped (non-printable),
                // but the aborting byte must be reprocessed.
                self.reset_to_ground();
                return false;
            }
        }
        true
    }

    /// Handle a byte in the EscapeIntermediate state (ESC + intermediate bytes).
    /// Returns false if the byte should be reprocessed from Ground.
    fn handle_escape_intermediate_byte(&mut self, byte: u8, tokens: &mut Vec<AnsiToken>) -> bool {
        match byte {
            0x20..=0x2f => {
                self.intermediate.push(byte);
            }
            0x30..=0x7e => {
                // Final byte — emit the complete escape sequence.
                tokens.push(AnsiToken::EscSequence {
                    intermediate: self.intermediate.clone(),
                    final_byte: byte,
                });
                self.reset_to_ground();
            }
            _ => {
                // Aborted escape-with-intermediate sequence.
                // ESC is non-printable, so silently drop
                // everything and reprocess the aborting byte.
                self.reset_to_ground();
                return false;
            }
        }
        true
    }

    /// Handle a byte in the CsiParam state (after ESC [).
    /// Returns false if the byte should be reprocessed from Ground.
    fn handle_csi_param_byte(&mut self, byte: u8, tokens: &mut Vec<AnsiToken>) -> bool {
        if self.parse_param_digit_or_separator(byte) {
            return true;
        }
        match byte {
            0x3c..=0x3f => {
                // Private mode indicator bytes: '<', '=', '>', '?'
                // Per ECMA-48 these are parameter bytes (0x30-0x3f).
                // Store in intermediate so the emulator can detect
                // DEC private modes (e.g. CSI ?25h, CSI ?1049h).
                self.intermediate.push(byte);
            }
            0x20..=0x2f => {
                self.flush_csi_param();
                self.intermediate.push(byte);
                self.state = ParseState::CsiIntermediate;
            }
            0x40..=0x7e => {
                self.flush_csi_param();
                tokens.push(AnsiToken::Csi {
                    params: self.csi_params.clone(),
                    intermediate: self.intermediate.clone(),
                    final_byte: byte,
                });
                self.reset_to_ground();
            }
            _ => {
                // CSI sequence aborted by an unexpected byte.
                // Emit the consumed '[' as text (so it is visible),
                // then reprocess the aborting byte from Ground.
                self.text_bytes.push(b'[');
                self.reset_to_ground();
                return false;
            }
        }
        true
    }

    /// Handle a byte in the CsiIntermediate state (CSI + intermediate bytes).
    /// Returns false if the byte should be reprocessed from Ground.
    fn handle_csi_intermediate_byte(&mut self, byte: u8, tokens: &mut Vec<AnsiToken>) -> bool {
        match byte {
            0x20..=0x2f => { self.intermediate.push(byte); }
            0x40..=0x7e => {
                tokens.push(AnsiToken::Csi {
                    params: self.csi_params.clone(),
                    intermediate: self.intermediate.clone(),
                    final_byte: byte,
                });
                self.reset_to_ground();
            }
            _ => {
                // CSI intermediate sequence aborted.
                // Emit the consumed '[' and intermediate bytes as text.
                self.text_bytes.push(b'[');
                for &ib in &self.intermediate {
                    self.text_bytes.push(ib);
                }
                self.reset_to_ground();
                return false;
            }
        }
        true
    }

    /// Handle a byte in the OscString state (after ESC ]).
    fn handle_osc_string_byte(&mut self, byte: u8, tokens: &mut Vec<AnsiToken>) -> bool {
        match byte {
            0x07 => {
                // BEL terminates OSC.
                let content = self.decode_string_content();
                tokens.push(AnsiToken::Osc(content));
                self.reset_to_ground();
            }
            0x1b => {
                // Potential ST (String Terminator) start — ESC.
                // Buffer it so the next byte can detect ESC + "\\".
                self.buffer.push(byte);
            }
            _ => {
                // If the previous byte was a buffered ESC, check
                // whether this byte completes an ST.
                if self.buffer.ends_with(&[0x1b]) {
                    self.buffer.pop(); // remove buffered ESC
                    if byte == b'\\' {
                        // ST — terminate OSC.
                        let content = self.decode_string_content();
                        tokens.push(AnsiToken::Osc(content));
                        self.reset_to_ground();
                    } else {
                        // ESC not followed by "\\" — treat the
                        // ESC as raw string data and continue.
                        self.push_string_byte(0x1b);
                        self.push_string_byte(byte);
                    }
                } else {
                    self.push_string_byte(byte);
                }
            }
        }
        true
    }

    /// Handle a byte in the DcsEntry state (after ESC P).
    /// Returns false if the byte should be reprocessed from Ground.
    fn handle_dcs_entry_byte(&mut self, byte: u8, _tokens: &mut Vec<AnsiToken>) -> bool {
        match byte {
            0x30..=0x39 | b';' | b':' => {
                // Parse the first parameter byte before transitioning.
                self.parse_param_digit_or_separator(byte);
                self.state = ParseState::DcsParam;
            }
            0x20..=0x2f => {
                self.state = ParseState::DcsIntermediate;
            }
            0x40..=0x7e => {
                // Final byte — record it and transition to
                // DcsString to collect the data payload that
                // follows the final byte.
                self.dcs_final_byte = byte;
                self.state = ParseState::DcsString;
            }
            0x3c..=0x3f => {
                self.intermediate.push(byte);
            }
            _ => {
                self.reset_to_ground();
                return false;
            }
        }
        true
    }

    /// Handle a byte in the DcsParam or DcsIntermediate state.
    /// Returns false if the byte should be reprocessed from Ground.
    fn handle_dcs_param_byte(&mut self, byte: u8, _tokens: &mut Vec<AnsiToken>) -> bool {
        if self.parse_param_digit_or_separator(byte) {
            return true;
        }
        match byte {
            0x20..=0x2f => {
                self.intermediate.push(byte);
            }
            0x40..=0x7e => {
                // Final byte — record it and transition to
                // DcsString to collect the data payload.
                self.flush_csi_param();
                self.dcs_final_byte = byte;
                self.state = ParseState::DcsString;
            }
            _ => {
                self.reset_to_ground();
                return false;
            }
        }
        true
    }

    /// Handle a byte in the DcsString state (collecting DCS data payload).
    fn handle_dcs_string_byte(&mut self, byte: u8, tokens: &mut Vec<AnsiToken>) -> bool {
        if byte == 0x1b {
            // Potential ST start — buffer for detection.
            self.buffer.push(byte);
        } else if self.buffer.ends_with(&[0x1b]) {
            self.buffer.pop(); // remove buffered ESC
            if byte == b'\\' {
                // ST — emit the complete DCS token.
                let content = self.decode_string_content();
                tokens.push(AnsiToken::Dcs {
                    params: self.csi_params.clone(),
                    intermediate: self.intermediate.clone(),
                    final_byte: self.dcs_final_byte,
                    data: content,
                });
                self.reset_to_ground();
            } else {
                // ESC not followed by "\\" — treat ESC as data.
                self.push_string_byte(0x1b);
                self.push_string_byte(byte);
            }
        } else {
            self.push_string_byte(byte);
        }
        true
    }

    /// Handle a byte in the String state (SOS, PM, APC — content discarded).
    fn handle_string_byte(&mut self, byte: u8) -> bool {
        if byte == 0x1b {
            self.buffer.push(byte);
        } else if self.buffer.ends_with(&[0x1b]) && byte == b'\\' {
            self.reset_to_ground();
        }
        true
    }

    // ── Main parser ──

    // State-machine parser: dispatches input bytes by current state.
    // States: Ground, Escape, CSI params, OSC, DCS, etc.
    // Extracted handlers: handle_* methods for each state transition.
    pub fn parse(&mut self, input: &[u8]) -> Vec<AnsiToken> {
        let mut tokens = Vec::new();

        // Use a manual index so that abort paths can reprocess the current
        // byte from Ground state instead of silently consuming it.
        let mut i = 0;
        while i < input.len() {
            let byte = input[i];
            let mut advance = true; // set to false to reprocess this byte

            // CAN (0x18) and SUB (0x1a) abort any in-flight escape/CSI/OSC/
            // DCS/string sequence, regardless of which state we are in.
            // They are emitted as Control tokens (same as in Ground state).
            if self.state != ParseState::Ground && (byte == 0x18 || byte == 0x1a) {
                self.reset_to_ground();
                tokens.push(AnsiToken::Control(byte));
            } else {
                advance = match self.state {
                    ParseState::Ground => {
                        self.process_ground_byte(byte, &mut tokens);
                        true
                    }
                    ParseState::Escape => {
                        self.handle_escape_byte(byte, &mut tokens)
                    }
                    ParseState::EscapeIntermediate => {
                        self.handle_escape_intermediate_byte(byte, &mut tokens)
                    }
                    ParseState::CsiParam => {
                        self.handle_csi_param_byte(byte, &mut tokens)
                    }
                    ParseState::CsiIntermediate => {
                        self.handle_csi_intermediate_byte(byte, &mut tokens)
                    }
                    ParseState::OscString => {
                        self.handle_osc_string_byte(byte, &mut tokens)
                    }
                    ParseState::DcsEntry => {
                        self.handle_dcs_entry_byte(byte, &mut tokens)
                    }
                    ParseState::DcsParam | ParseState::DcsIntermediate => {
                        self.handle_dcs_param_byte(byte, &mut tokens)
                    }
                    ParseState::DcsString => {
                        self.handle_dcs_string_byte(byte, &mut tokens)
                    }
                    ParseState::String => {
                        self.handle_string_byte(byte)
                    }
                };
            }

            if advance {
                i += 1;
            }
        }

        self.flush_text_bytes_preserve_incomplete(&mut tokens);
        tokens
    }
    /// Flush any remaining buffered text and discard incomplete sequences.
    /// Call this when the input stream ends (e.g., PTY closed) to ensure
    /// no trailing text is lost.
    pub fn finish(&mut self) -> Vec<AnsiToken> {
        let mut tokens = Vec::new();
        match self.state {
            ParseState::Ground => {
                self.flush_text_bytes(&mut tokens);
            }
            ParseState::Escape => {
                // Incomplete ESC — ESC is non-printable, silently drop.
                self.reset_to_ground();
            }
            ParseState::CsiParam | ParseState::CsiIntermediate => {
                // Incomplete CSI — emit the consumed '[' as visible text,
                // matching the abort behaviour in the main parse loop.
                self.text_bytes.push(b'[');
                self.reset_to_ground();
                self.flush_text_bytes(&mut tokens);
            }
            ParseState::EscapeIntermediate => {
                // Incomplete ESC-with-intermediate — ESC is non-printable,
                // intermediate bytes are discarded.
                self.reset_to_ground();
            }
            ParseState::OscString | ParseState::DcsString
            | ParseState::String | ParseState::DcsEntry
            | ParseState::DcsParam | ParseState::DcsIntermediate => {
                // Incomplete string/DCS — discard entirely.
                self.reset_to_ground();
            }
        }
        tokens
    }

    pub fn parse_string(&mut self, input: &str) -> Vec<AnsiToken> {
        self.parse(input.as_bytes())
    }

    fn flush_csi_param(&mut self) {
        if self.current_param.is_empty() {
            self.csi_params.push(vec![0]);
        } else {
            self.csi_params.push(self.current_param.clone());
            self.current_param.clear();
        }
    }

    fn reset_to_ground(&mut self) {
        self.state = ParseState::Ground;
        self.buffer.clear();
        self.csi_params.clear();
        self.current_param.clear();
        self.intermediate.clear();
        self.string_content.clear();
        self.dcs_final_byte = 0;
        // NOTE: text_bytes is NOT cleared here — it is only cleared when
        // explicitly flushed via flush_text_bytes().  reset_to_ground()
        // is called from within CSI/OSC/DCS handling where text_bytes
        // have already been flushed before entering the escape sequence.
    }
}

pub fn parse_ansi(input: &[u8]) -> Vec<AnsiToken> {
    let mut parser = AnsiParser::new();
    parser.parse(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_plain_text() {
        let tokens = parse_ansi(b"Hello World");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0], AnsiToken::Text(s) if s == "Hello World"));
    }

    #[test]
    fn test_parse_control_chars() {
        let tokens = parse_ansi(b"Hello\nWorld\r\t");
        // Note: 0x0a (LF), 0x0d (CR), 0x09 (TAB) are treated as text by the parser
        // because they fall outside the control byte range 0x00-0x08, 0x0b-0x0c, 0x0e-0x1f.
        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0], AnsiToken::Text(s) if s == "Hello\nWorld\r\t"));
    }

    #[test]
    fn test_parse_csi_cursor_move() {
        let tokens = parse_ansi(b"\x1b[10;20H");
        assert_eq!(tokens.len(), 1);
        if let AnsiToken::Csi { params, intermediate, final_byte } = &tokens[0] {
            assert_eq!(params, &vec![vec![10], vec![20]]);
            assert!(intermediate.is_empty());
            assert_eq!(*final_byte, b'H');
        }
    }

    #[test]
    fn test_parse_csi_private_mode() {
        // CSI ?1049h — DEC private mode (alternate screen)
        let tokens = parse_ansi(b"\x1b[?1049h");
        assert_eq!(tokens.len(), 1);
        if let AnsiToken::Csi { params, intermediate, final_byte } = &tokens[0] {
            assert_eq!(params, &vec![vec![1049]]);
            assert_eq!(intermediate, &[b'?']);
            assert_eq!(*final_byte, b'h');
        }
    }

    #[test]
    fn test_parse_csi_private_mode_cursor() {
        // CSI ?25l — hide cursor (DEC private mode)
        let tokens = parse_ansi(b"\x1b[?25l");
        assert_eq!(tokens.len(), 1);
        if let AnsiToken::Csi { params, intermediate, final_byte } = &tokens[0] {
            assert_eq!(params, &vec![vec![25]]);
            assert_eq!(intermediate, &[b'?']);
            assert_eq!(*final_byte, b'l');
        }
    }

    #[test]
    fn test_parse_csi_sgr_colors() {
        let tokens = parse_ansi(b"\x1b[38;2;255;128;64m");
        assert_eq!(tokens.len(), 1);
        if let AnsiToken::Csi { params, .. } = &tokens[0] {
            assert_eq!(params, &vec![vec![38], vec![2], vec![255], vec![128], vec![64]]);
        }
    }

    #[test]
    fn test_parse_mixed() {
        let tokens = parse_ansi(b"Hello\x1b[31mRed\x1b[0mNormal");
        assert_eq!(tokens.len(), 5);
        assert!(matches!(&tokens[0], AnsiToken::Text(s) if s == "Hello"));
        assert!(matches!(&tokens[1], AnsiToken::Csi { final_byte: b'm', .. }));
    }

    #[test]
    fn test_streaming_parser() {
        let mut parser = AnsiParser::new();
        let tokens1 = parser.parse(b"Hello\x1b[");
        assert_eq!(tokens1.len(), 1);
        assert!(matches!(&tokens1[0], AnsiToken::Text(s) if s == "Hello"));
        let tokens2 = parser.parse(b"31mWorld");
        assert_eq!(tokens2.len(), 2);
        assert!(matches!(&tokens2[0], AnsiToken::Csi { final_byte: b'm', .. }));
    }

    #[test]
    fn test_parse_escape_simple() {
        let tokens = parse_ansi(b"\x1b7");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0], AnsiToken::Escape(b'7')));
    }

    #[test]
    fn test_parse_utf8_multibyte() {
        // Box-drawing character '|' is U+2502, encoded as 0xE2 0x94 0x82
        let tokens = parse_ansi("\u{2502}".as_bytes());
        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0], AnsiToken::Text(s) if s == "\u{2502}"));
    }

    #[test]
    fn test_parse_utf8_mixed_ascii() {
        // Mix ASCII and multi-byte UTF-8 (box-drawing chars used by htop)
        let input = "CPU\u{2502}MEM\u{2502}SWP";
        let tokens = parse_ansi(input.as_bytes());
        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0], AnsiToken::Text(s) if s == input));
    }

    #[test]
    fn test_parse_utf8_with_escape() {
        // UTF-8 text followed by an escape sequence
        let input = "\u{2500}\u{253c}\u{2500}\x1b[31m";
        let tokens = parse_ansi(input.as_bytes());
        assert_eq!(tokens.len(), 2);
        assert!(matches!(&tokens[0], AnsiToken::Text(s) if s == "\u{2500}\u{253c}\u{2500}"));
        assert!(matches!(&tokens[1], AnsiToken::Csi { final_byte: b'm', .. }));
    }

    #[test]
    fn test_parse_utf8_split_across_calls() {
        // Simulate a 3-byte UTF-8 character split across two parse() calls
        let mut parser = AnsiParser::new();
        // U+2502 = [0xE2, 0x94, 0x82]
        let tokens1 = parser.parse(&[0xE2, 0x94]);
        assert_eq!(tokens1.len(), 0); // Not enough bytes for a complete char yet
        let tokens2 = parser.parse(&[0x82]);
        assert_eq!(tokens2.len(), 1);
        assert!(matches!(&tokens2[0], AnsiToken::Text(s) if s == "\u{2502}"));
    }

    #[test]
    fn test_csi_aborted_by_control_char() {
        // ESC [ followed by LF (0x0a) — CSI sequence is aborted.
        // The '[' should be emitted as text, and the LF should be emitted
        // as text (it's in the text range, not control range, in Ground state).
        let tokens = parse_ansi(b"\x1b[\nHello");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0], AnsiToken::Text(s) if s == "[\nHello"));
    }

    #[test]
    fn test_csi_aborted_by_high_byte() {
        // ESC [ followed by a high byte (0xff) that's not a CSI byte.
        // High bytes fall through to the default abort path in CsiParam.
        // The '[' is emitted as text. The 0xff stays in text_bytes
        // and is dropped by flush_text_bytes_preserve_incomplete (since
        // it's an invalid UTF-8 start byte). The remaining 'X' is flushed
        // in the next call.
        let input: &[u8] = b"\x1b[\xffX";
        let mut parser = AnsiParser::new();
        let tokens = parser.parse(input);
        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0], AnsiToken::Text(s) if s == "["));
        // 'X' is still in text_bytes; feed 'Y' to flush it
        let tokens2 = parser.parse(b"Y");
        assert_eq!(tokens2.len(), 1);
        assert!(matches!(&tokens2[0], AnsiToken::Text(s) if s == "XY"));
    }

    #[test]
    fn test_escape_aborted_by_control() {
        // ESC followed by a non-ESC-sequence byte (0x01 = SOH control).
        // The ESC is dropped (non-printable), and 0x01 is reprocessed.
        let tokens = parse_ansi(b"\x1b\x01A");
        // 0x01 is in the control range (0x00..=0x08), so it's a Control token.
        // 'A' is text.
        assert_eq!(tokens.len(), 2);
        assert!(matches!(&tokens[0], AnsiToken::Control(0x01)));
        assert!(matches!(&tokens[1], AnsiToken::Text(s) if s == "A"));
    }

    #[test]
    fn test_csi_valid_then_aborted() {
        // A valid CSI sequence followed by an aborted one:
        // ESC[31m (valid) then ESC[ + LF (aborted)
        let tokens = parse_ansi(b"\x1b[31m\x1b[\nRed");
        // Token 1: CSI 31m
        // Token 2: Text "[\nRed"
        assert_eq!(tokens.len(), 2);
        assert!(matches!(&tokens[0], AnsiToken::Csi { final_byte: b'm', .. }));
        assert!(matches!(&tokens[1], AnsiToken::Text(s) if s == "[\nRed"));
    }

    // ---- New tests for the fixed bugs ----

    #[test]
    fn test_escape_with_intermediate_charset() {
        // ESC ( B — designate G0 as ASCII
        let tokens = parse_ansi(b"\x1b(B");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0], AnsiToken::EscSequence {
            intermediate,
            final_byte: b'B',
        } if intermediate == &[b'(']));
    }

    #[test]
    fn test_escape_with_intermediate_decdblh() {
        // ESC # 3 — DECDHL double-height top
        let tokens = parse_ansi(b"\x1b#3");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0], AnsiToken::EscSequence {
            intermediate,
            final_byte: b'3',
        } if intermediate == &[b'#']));
    }

    #[test]
    fn test_escape_with_intermediate_not_csi() {
        // ESC ( B should NOT be emitted as Csi.
        let tokens = parse_ansi(b"\x1b(B");
        assert!(!matches!(&tokens[0], AnsiToken::Csi { .. }));
    }

    #[test]
    fn test_escape_with_two_intermediates() {
        // ESC SP F — S7C1T (two intermediates: space + nothing... actually just one)
        // Let's test ESC SP F properly: SP is intermediate, F is final.
        let tokens = parse_ansi(b"\x1b F");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0], AnsiToken::EscSequence {
            intermediate,
            final_byte: b'F',
        } if intermediate == &[b' ']));
    }

    #[test]
    fn test_escape_intermediate_aborted() {
        // ESC ( followed by 0x01 (control) — abort, reprocess 0x01 from Ground.
        let tokens = parse_ansi(b"\x1b(\x01A");
        // ESC ( is non-printable and dropped, 0x01 is a Control, 'A' is text.
        assert_eq!(tokens.len(), 2);
        assert!(matches!(&tokens[0], AnsiToken::Control(0x01)));
        assert!(matches!(&tokens[1], AnsiToken::Text(s) if s == "A"));
    }

    #[test]
    fn test_dcs_basic() {
        // DCS: ESC P <params> <final> <data> ST
        // ESC P 1 $ r <data> ESC \  (DECRQSS — request setting)
        let input = b"\x1bP1$r\x1b\\";
        let tokens = parse_ansi(input);
        assert_eq!(tokens.len(), 1);
        if let AnsiToken::Dcs { params, intermediate, final_byte, data } = &tokens[0] {
            assert_eq!(params, &vec![vec![1]]);
            assert_eq!(intermediate, &[b'$']);
            assert_eq!(*final_byte, b'r');
            assert_eq!(data, "");
        } else {
            panic!("Expected Dcs token, got {:?}", tokens[0]);
        }
    }

    #[test]
    fn test_dcs_with_data() {
        // DCS with payload: ESC P <final> <data> ESC \
        let input = b"\x1bPqhello world\x1b\\";
        let tokens = parse_ansi(input);
        assert_eq!(tokens.len(), 1);
        if let AnsiToken::Dcs { final_byte, data, .. } = &tokens[0] {
            assert_eq!(*final_byte, b'q');
            assert_eq!(data, "hello world");
        } else {
            panic!("Expected Dcs token, got {:?}", tokens[0]);
        }
    }

    #[test]
    fn test_dcs_data_after_params() {
        // DCS with params and data: ESC P 1 ; 2 r <data> ESC \
        let input = b"\x1bP1;2rABCDEF\x1b\\";
        let tokens = parse_ansi(input);
        assert_eq!(tokens.len(), 1);
        if let AnsiToken::Dcs { params, final_byte, data, .. } = &tokens[0] {
            assert_eq!(params, &vec![vec![1], vec![2]]);
            assert_eq!(*final_byte, b'r');
            assert_eq!(data, "ABCDEF");
        } else {
            panic!("Expected Dcs token, got {:?}", tokens[0]);
        }
    }

    #[test]
    fn test_dcs_final_byte_recorded_correctly() {
        // Verify the final byte is NOT hardcoded — test with different final bytes.
        let input = b"\x1bPpdata\x1b\\";
        let tokens = parse_ansi(input);
        assert_eq!(tokens.len(), 1);
        if let AnsiToken::Dcs { final_byte, .. } = &tokens[0] {
            assert_eq!(*final_byte, b'p');
        } else {
            panic!("Expected Dcs token");
        }
    }

    #[test]
    fn test_dcs_string_content_not_stale() {
        // DCS should not reuse stale string_content from a previous OSC.
        let mut parser = AnsiParser::new();
        // First, parse an OSC that leaves content.
        parser.parse(b"\x1b]0;title\x07");
        // Now parse a DCS with no data.
        let tokens = parser.parse(b"\x1bPq\x1b\\");
        assert_eq!(tokens.len(), 1);
        if let AnsiToken::Dcs { data, .. } = &tokens[0] {
            assert_eq!(data, "");
        } else {
            panic!("Expected Dcs token");
        }
    }

    #[test]
    fn test_dcs_bel_not_terminator() {
        // BEL (0x07) does NOT terminate DCS — only ST does.
        let input = b"\x1bPq\x07data\x1b\\";
        let tokens = parse_ansi(input);
        assert_eq!(tokens.len(), 1);
        if let AnsiToken::Dcs { data, .. } = &tokens[0] {
            // BEL (0x07) is inside the data, so data should contain it.
            assert_eq!(data, "\x07data");
        } else {
            panic!("Expected Dcs token");
        }
    }

    #[test]
    fn test_osc_with_utf8() {
        // OSC with UTF-8 title: ESC ] 0 ; héllo BEL
        // "héllo" in UTF-8: h(0x68) é(0xC3 0xA9) l(0x6C) l(0x6C) o(0x6F)
        let input: &[u8] = b"\x1b]0;h\xC3\xA9llo\x07";
        let tokens = parse_ansi(input);
        assert_eq!(tokens.len(), 1);
        if let AnsiToken::Osc(content) = &tokens[0] {
            // "0;h" + é (properly decoded from UTF-8 bytes) + "llo"
            assert!(content.starts_with("0;h"));
            assert!(content.ends_with("llo"));
            assert!(content.contains('\u{e9}'));
            assert_eq!(content, "0;h\u{e9}llo");
        } else {
            panic!("Expected Osc token");
        }
    }

    #[test]
    fn test_osc_st_terminator() {
        // OSC terminated by ST (ESC \)
        let input = b"\x1b]0;title\x1b\\";
        let tokens = parse_ansi(input);
        assert_eq!(tokens.len(), 1);
        if let AnsiToken::Osc(content) = &tokens[0] {
            assert_eq!(content, "0;title");
        } else {
            panic!("Expected Osc token");
        }
    }

    #[test]
    fn test_osc_esc_without_backslash() {
        // ESC inside OSC not followed by \ — ESC should be treated as data.
        let input = b"\x1b]0;ESC\x1bnot-st\x07";
        let tokens = parse_ansi(input);
        assert_eq!(tokens.len(), 1);
        if let AnsiToken::Osc(content) = &tokens[0] {
            // The ESC (0x1b) should be raw data inside the string, not trigger ST.
            assert_eq!(content, "0;ESC\x1bnot-st");
        } else {
            panic!("Expected Osc token");
        }
    }

    #[test]
    fn test_can_aborts_csi() {
        // CAN (0x18) aborts an in-flight CSI sequence.
        let input = b"\x1b[31\x18After";
        let tokens = parse_ansi(input);
        // CSI is aborted, CAN is emitted as Control, then text "After".
        assert!(tokens.iter().any(|t| matches!(t, AnsiToken::Control(0x18))));
        assert!(tokens.iter().any(|t| matches!(t, AnsiToken::Text(s) if s == "After")));
    }

    #[test]
    fn test_sub_aborts_osc() {
        // SUB (0x1a) aborts an in-flight OSC sequence.
        let input = b"\x1b]0;title\x1aAfter";
        let tokens = parse_ansi(input);
        assert!(tokens.iter().any(|t| matches!(t, AnsiToken::Control(0x1a))));
        assert!(tokens.iter().any(|t| matches!(t, AnsiToken::Text(s) if s == "After")));
    }

    #[test]
    fn test_can_aborts_escape() {
        // CAN (0x18) aborts an in-flight ESC sequence.
        let input = b"\x1b\x18After";
        let tokens = parse_ansi(input);
        assert!(tokens.iter().any(|t| matches!(t, AnsiToken::Control(0x18))));
        assert!(tokens.iter().any(|t| matches!(t, AnsiToken::Text(s) if s == "After")));
    }

    #[test]
    fn test_can_aborts_dcs() {
        // CAN (0x18) aborts an in-flight DCS sequence.
        let input = b"\x1bPqdata\x18After";
        let tokens = parse_ansi(input);
        assert!(tokens.iter().any(|t| matches!(t, AnsiToken::Control(0x18))));
        assert!(tokens.iter().any(|t| matches!(t, AnsiToken::Text(s) if s == "After")));
    }

    #[test]
    fn test_sub_aborts_dcs_string() {
        // SUB (0x1a) aborts while collecting DCS string data.
        let input = b"\x1bPqdata\x1aAfter";
        let tokens = parse_ansi(input);
        assert!(tokens.iter().any(|t| matches!(t, AnsiToken::Control(0x1a))));
        assert!(tokens.iter().any(|t| matches!(t, AnsiToken::Text(s) if s == "After")));
    }

    #[test]
    fn test_can_aborts_escape_intermediate() {
        // CAN (0x18) aborts an in-flight ESC-with-intermediate sequence.
        let input = b"\x1b(\x18After";
        let tokens = parse_ansi(input);
        assert!(tokens.iter().any(|t| matches!(t, AnsiToken::Control(0x18))));
        assert!(tokens.iter().any(|t| matches!(t, AnsiToken::Text(s) if s == "After")));
    }

    #[test]
    fn test_finish_flushes_text() {
        let mut parser = AnsiParser::new();
        // finish() on a fresh parser returns nothing.
        let tokens = parser.finish();
        assert!(tokens.is_empty());

        // parse() already flushes all text, so finish() returns nothing.
        parser.parse(b"Hello");
        let tokens = parser.finish();
        assert!(tokens.is_empty());

        // The real use of finish() is for incomplete sequences.
        // Verify that calling finish() twice is safe (idempotent).
        let tokens = parser.finish();
        assert!(tokens.is_empty());
    }

    #[test]
    fn test_finish_discards_incomplete_csi() {
        let mut parser = AnsiParser::new();
        parser.parse(b"Before\x1b[31");
        // parse() already emitted "Before" as text.  We're now in CsiParam
        // state with partial parameter "31".  finish() should emit the
        // consumed '[' as visible text (matching the abort behaviour).
        let tokens = parser.finish();
        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0], AnsiToken::Text(s) if s == "["));
    }

    #[test]
    fn test_finish_discards_incomplete_osc() {
        let mut parser = AnsiParser::new();
        parser.parse(b"\x1b]0;title");
        let tokens = parser.finish();
        // Incomplete OSC is discarded.
        assert!(tokens.is_empty());
    }

    #[test]
    fn test_finish_discards_incomplete_dcs() {
        let mut parser = AnsiParser::new();
        parser.parse(b"\x1bPqdata");
        let tokens = parser.finish();
        // Incomplete DCS is discarded.
        assert!(tokens.is_empty());
    }

    #[test]
    fn test_finish_discards_incomplete_escape() {
        let mut parser = AnsiParser::new();
        parser.parse(b"\x1b");
        let tokens = parser.finish();
        // Incomplete ESC is silently dropped.
        assert!(tokens.is_empty());
    }

    #[test]
    fn test_finish_discards_incomplete_esc_intermediate() {
        let mut parser = AnsiParser::new();
        parser.parse(b"\x1b(");
        let tokens = parser.finish();
        // Incomplete ESC-with-intermediate is discarded.
        assert!(tokens.is_empty());
    }

    #[test]
    fn test_string_length_limit_osc() {
        // Feed an OSC with more than MAX_STRING_LENGTH bytes without terminator.
        let long_data: Vec<u8> = (0..MAX_STRING_LENGTH + 100).map(|_| b'X').collect();
        let input: Vec<u8> = [
            b"\x1b]0;".to_vec(),
            long_data,
        ].concat();
        let tokens = parse_ansi(&input);
        // The OSC was never terminated, so no token emitted.
        assert!(tokens.is_empty());
        // But string_content should be capped at MAX_STRING_LENGTH.
        // We can't inspect it directly, but we verify no panic and correct behavior.
    }

    #[test]
    fn test_dcs_streaming() {
        // DCS split across multiple parse() calls.
        let mut parser = AnsiParser::new();
        let t1 = parser.parse(b"\x1bP1$r");
        assert!(t1.is_empty()); // No token yet, collecting data

        let t2 = parser.parse(b"he");
        assert!(t2.is_empty()); // Still collecting

        let t3 = parser.parse(b"llo\x1b\\");
        assert_eq!(t3.len(), 1);
        if let AnsiToken::Dcs { params, final_byte, data, .. } = &t3[0] {
            assert_eq!(params, &vec![vec![1]]);
            assert_eq!(*final_byte, b'r');
            assert_eq!(data, "hello");
        } else {
            panic!("Expected Dcs token");
        }
    }
}
