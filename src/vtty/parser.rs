/// ANSI/VT100 escape sequence parser.
/// Streaming byte parser implemented as a state machine.

#[derive(Debug, Clone, PartialEq)]
pub enum AnsiToken {
    Text(String),
    Control(u8),
    Csi { params: Vec<Vec<u16>>, intermediate: Vec<u8>, final_byte: u8 },
    Osc(String),
    Escape(u8),
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
}

pub struct AnsiParser {
    state: ParseState,
    buffer: Vec<u8>,
    csi_params: Vec<Vec<u16>>,
    current_param: Vec<u16>,
    intermediate: Vec<u8>,
    string_content: String,
    /// Raw byte buffer for accumulating UTF-8 text in Ground state.
    /// Decoded to a String only when a non-text token forces a flush.
    text_bytes: Vec<u8>,
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
            string_content: String::new(),
            text_bytes: Vec::new(),
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

    pub fn parse(&mut self, input: &[u8]) -> Vec<AnsiToken> {
        let mut tokens = Vec::new();

        for &byte in input {
            match self.state {
                ParseState::Ground => {
                    match byte {
                        0x1b => {
                            self.flush_text_bytes(&mut tokens);
                            self.state = ParseState::Escape;
                            self.buffer.clear();
                            self.buffer.push(byte);
                        }
                        0x00..=0x08 | 0x0b..=0x0c | 0x0e..=0x1a | 0x1c..=0x1f => {
                            self.flush_text_bytes(&mut tokens);
                            tokens.push(AnsiToken::Control(byte));
                        }
                        0x7f => {}
                        _ => { self.text_bytes.push(byte); }
                    }
                }

                ParseState::Escape => {
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
                        }
                        b'X' | b'^' | b'_' => {
                            self.state = ParseState::String;
                        }
                        0x20..=0x2f => {
                            self.intermediate.clear();
                            self.intermediate.push(byte);
                            self.state = ParseState::CsiIntermediate;
                        }
                        0x30..=0x7e => {
                            tokens.push(AnsiToken::Escape(byte));
                            self.reset_to_ground();
                        }
                        _ => { self.reset_to_ground(); }
                    }
                }

                ParseState::CsiParam => {
                    match byte {
                        0x30..=0x39 => {
                            if let Some(last) = self.current_param.last_mut() {
                                *last = (*last * 10) + ((byte - b'0') as u16);
                            } else {
                                self.current_param.push((byte - b'0') as u16);
                            }
                        }
                        b';' => {
                            if self.current_param.is_empty() {
                                self.csi_params.push(vec![0]);
                            } else {
                                self.csi_params.push(self.current_param.clone());
                                self.current_param.clear();
                            }
                        }
                        b':' => {
                            self.current_param.push(0);
                        }
                        0x3c..=0x3f => {
                            // Private mode indicator bytes: '<', '=', '>', '?'
                            // Per ECMA-48 these are parameter bytes (0x30-0x3f).
                            // Store in intermediate so the emulator can detect
                            // DEC private modes (e.g. CSI ?25h, CSI ?1049h).
                            self.intermediate.push(byte);
                            // Stay in CsiParam to continue parsing parameter digits.
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
                        _ => { self.reset_to_ground(); }
                    }
                }

                ParseState::CsiIntermediate => {
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
                        _ => { self.reset_to_ground(); }
                    }
                }

                ParseState::OscString => {
                    match byte {
                        0x07 => {
                            tokens.push(AnsiToken::Osc(self.string_content.clone()));
                            self.reset_to_ground();
                        }
                        0x1b => {
                            // Potential ST (String Terminator) start — ESC
                            // Push to buffer so the next byte can detect ESC + "\"
                            self.buffer.push(byte);
                        }
                        _ => {
                            if self.buffer.ends_with(&[0x1b]) && byte == b'\\' {
                                tokens.push(AnsiToken::Osc(self.string_content.clone()));
                                self.reset_to_ground();
                            } else {
                                self.string_content.push(byte as char);
                            }
                        }
                    }
                }

                ParseState::DcsEntry => {
                    match byte {
                        0x30..=0x39 | b';' | b':' => {
                            // Parse the first parameter byte before transitioning
                            if byte.is_ascii_digit() {
                                if let Some(last) = self.current_param.last_mut() {
                                    *last = (*last * 10) + ((byte - b'0') as u16);
                                } else {
                                    self.current_param.push((byte - b'0') as u16);
                                }
                            } else if byte == b';' {
                                if self.current_param.is_empty() {
                                    self.csi_params.push(vec![0]);
                                } else {
                                    self.csi_params.push(self.current_param.clone());
                                    self.current_param.clear();
                                }
                            } else if byte == b':' {
                                self.current_param.push(0);
                            }
                            self.state = ParseState::DcsParam;
                        }
                        0x20..=0x2f => { self.state = ParseState::DcsIntermediate; }
                        0x40..=0x7e => { self.state = ParseState::DcsString; }
                        0x3c..=0x3f => {
                            self.intermediate.push(byte);
                        }
                        _ => { self.reset_to_ground(); }
                    }
                }

                ParseState::DcsParam | ParseState::DcsIntermediate => {
                    match byte {
                        0x30..=0x39 => {
                            if let Some(last) = self.current_param.last_mut() {
                                *last = (*last * 10) + ((byte - b'0') as u16);
                            } else {
                                self.current_param.push((byte - b'0') as u16);
                            }
                        }
                        b';' => {
                            if self.current_param.is_empty() {
                                self.csi_params.push(vec![0]);
                            } else {
                                self.csi_params.push(self.current_param.clone());
                                self.current_param.clear();
                            }
                        }
                        b':' => {
                            self.current_param.push(0);
                        }
                        0x40..=0x7e => {
                            // Flush any pending parameter, then transition to DcsString
                            self.flush_csi_param();
                            tokens.push(AnsiToken::Dcs {
                                params: self.csi_params.clone(),
                                intermediate: self.intermediate.clone(),
                                final_byte: byte,
                                data: self.string_content.clone(),
                            });
                            self.reset_to_ground();
                        }
                        0x20..=0x2f => { self.intermediate.push(byte); }
                        _ => { self.reset_to_ground(); }
                    }
                }

                ParseState::DcsString => {
                    if byte == 0x1b {
                        // Potential ST start — store for detection
                        self.buffer.push(byte);
                    } else if self.buffer.ends_with(&[0x1b]) && byte == b'\\' {
                        tokens.push(AnsiToken::Dcs {
                            params: self.csi_params.clone(),
                            intermediate: self.intermediate.clone(),
                            final_byte: b'@',
                            data: self.string_content.clone(),
                        });
                        self.reset_to_ground();
                    } else {
                        self.string_content.push(byte as char);
                    }
                }

                ParseState::String => {
                    if byte == 0x1b {
                        // Potential ST start — store for detection
                        self.buffer.push(byte);
                    } else if self.buffer.ends_with(&[0x1b]) && byte == b'\\' {
                        self.reset_to_ground();
                    }
                }
            }
        }

        self.flush_text_bytes_preserve_incomplete(&mut tokens);
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
        // Box-drawing character '│' is U+2502, encoded as 0xE2 0x94 0x82
        let tokens = parse_ansi("│".as_bytes());
        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0], AnsiToken::Text(s) if s == "│"));
    }

    #[test]
    fn test_parse_utf8_mixed_ascii() {
        // Mix ASCII and multi-byte UTF-8 (box-drawing chars used by htop)
        let input = "CPU│MEM│SWP";
        let tokens = parse_ansi(input.as_bytes());
        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0], AnsiToken::Text(s) if s == input));
    }

    #[test]
    fn test_parse_utf8_with_escape() {
        // UTF-8 text followed by an escape sequence
        let input = "─┼─\x1b[31m";
        let tokens = parse_ansi(input.as_bytes());
        assert_eq!(tokens.len(), 2);
        assert!(matches!(&tokens[0], AnsiToken::Text(s) if s == "─┼─"));
        assert!(matches!(&tokens[1], AnsiToken::Csi { final_byte: b'm', .. }));
    }

    #[test]
    fn test_parse_utf8_split_across_calls() {
        // Simulate a 3-byte UTF-8 character split across two parse() calls
        let mut parser = AnsiParser::new();
        // '│' = [0xE2, 0x94, 0x82]
        let tokens1 = parser.parse(&[0xE2, 0x94]);
        assert_eq!(tokens1.len(), 0); // Not enough bytes for a complete char yet
        let tokens2 = parser.parse(&[0x82]);
        assert_eq!(tokens2.len(), 1);
        assert!(matches!(&tokens2[0], AnsiToken::Text(s) if s == "│"));
    }
}
