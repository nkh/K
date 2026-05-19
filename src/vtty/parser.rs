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
        }
    }

    pub fn parse(&mut self, input: &[u8]) -> Vec<AnsiToken> {
        let mut tokens = Vec::new();
        let mut text_buffer = String::new();

        for &byte in input {
            match self.state {
                ParseState::Ground => {
                    match byte {
                        0x1b => {
                            if !text_buffer.is_empty() {
                                tokens.push(AnsiToken::Text(text_buffer.clone()));
                                text_buffer.clear();
                            }
                            self.state = ParseState::Escape;
                            self.buffer.clear();
                            self.buffer.push(byte);
                        }
                        0x00..=0x08 | 0x0b..=0x0c | 0x0e..=0x1a | 0x1c..=0x1f => {
                            if !text_buffer.is_empty() {
                                tokens.push(AnsiToken::Text(text_buffer.clone()));
                                text_buffer.clear();
                            }
                            tokens.push(AnsiToken::Control(byte));
                        }
                        0x7f => {}
                        _ => { text_buffer.push(byte as char); }
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
                            // Potential ST start, store and check next byte
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
                            self.state = ParseState::DcsParam;
                        }
                        0x20..=0x2f => { self.state = ParseState::DcsIntermediate; }
                        0x40..=0x7e => { self.state = ParseState::DcsString; }
                        _ => { self.reset_to_ground(); }
                    }
                }

                ParseState::DcsParam | ParseState::DcsIntermediate => {
                    match byte {
                        0x40..=0x7e => { self.state = ParseState::DcsString; }
                        _ => {}
                    }
                }

                ParseState::DcsString => {
                    if byte == 0x1b {
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
                    } else if self.buffer.ends_with(&[0x1b]) && byte == b'\\' {
                        self.reset_to_ground();
                    }
                }
            }
        }

        if !text_buffer.is_empty() {
            tokens.push(AnsiToken::Text(text_buffer));
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
        assert_eq!(tokens.len(), 5);
        assert!(matches!(&tokens[0], AnsiToken::Text(s) if s == "Hello"));
        assert!(matches!(&tokens[1], AnsiToken::Control(0x0a)));
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
}
