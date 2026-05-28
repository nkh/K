//! Key name parsing and binding resolution.
//!
//! Converts human-readable key names like `ctrl+left`, `f12`, or `enter` into
//! raw byte sequences that the terminal actually produces.  Also resolves a
//! `KeybindingsConfig` into a `Vec<(Vec<u8>, Action)>` lookup table.

use crate::config::schema::KeybindingsConfig;

/// The actions that can be bound to key sequences.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Action {
    NextCommand,
    PrevCommand,
    ToggleLog,
    SpawnCommand,
    ShowHelp,
    KillCommand,
    TogglePause,
    Quit,
}

impl Action {
    /// Human-readable description for the help overlay.
    pub fn description(&self) -> &'static str {
        match self {
            Action::NextCommand => "Switch to next command",
            Action::PrevCommand => "Switch to previous command",
            Action::ToggleLog => "Toggle command log overlay",
            Action::SpawnCommand => "Spawn a new command",
            Action::ShowHelp => "Show this help screen",
            Action::KillCommand => "Kill the active command",
            Action::TogglePause => "Pause / resume the active command",
            Action::Quit => "Quit the display",
        }
    }
}

/// A resolved binding: raw bytes → action.
#[derive(Debug, Clone)]
pub struct Binding {
    pub bytes: Vec<u8>,
    pub action: Action,
}

/// Parse a human-readable key name into its raw byte sequence.
///
/// Supports:
/// - Single characters: `a`, `1`, `@`
/// - Control keys: `ctrl+a` .. `ctrl+z`, `ctrl+@`, `ctrl+[`, `ctrl+\\`, etc.
/// - Alt/Meta keys: `alt+a` .. `alt+z`, `alt+0`, etc.
/// - Modified arrows: `ctrl+left`, `ctrl+right`, `ctrl+up`, `ctrl+down`,
///   `shift+left`, etc.
/// - Function keys: `f1` .. `f12`
/// - Special keys: `enter`/`return`, `tab`, `backspace`, `delete`, `insert`,
///   `home`, `end`, `pageup`, `pagedown`, `up`, `down`, `left`, `right`,
///   `esc`/`escape`
///
/// Falls back to treating the string as a raw escape sequence (with `\x1b`,
/// `\x0c`, etc.) for backward compatibility.
pub fn parse_key_name(name: &str) -> Option<Vec<u8>> {
    let lower = name.to_ascii_lowercase();

    // ── Compound modifiers ──
    if let Some(rest) = lower.strip_prefix("ctrl+") {
        return parse_ctrl(rest.trim());
    }
    if let Some(rest) = lower.strip_prefix("alt+") {
        return parse_alt(rest.trim());
    }
    if let Some(rest) = lower.strip_prefix("shift+") {
        return parse_shift(rest.trim());
    }

    // ── Function keys ──
    if let Some(n) = lower.strip_prefix("f") {
        if let Ok(d) = n.parse::<u8>() {
            if (1..=12).contains(&d) {
                return Some(fn_key_bytes(d));
            }
        }
    }

    // ── Named special keys ──
    match lower.as_str() {
        "enter" | "return" => return Some(vec![0x0d]),
        "tab" => return Some(vec![0x09]),
        "backspace" => return Some(vec![0x7f]),
        "delete" => return Some(vec![0x1b, b'[', b'3', b'~']),
        "insert" => return Some(vec![0x1b, b'[', b'2', b'~']),
        "home" => return Some(vec![0x1b, b'[', b'H']),
        "end" => return Some(vec![0x1b, b'[', b'F']),
        "pageup" | "page_up" => return Some(vec![0x1b, b'[', b'5', b'~']),
        "pagedown" | "page_down" => return Some(vec![0x1b, b'[', b'6', b'~']),
        "up" => return Some(vec![0x1b, b'[', b'A']),
        "down" => return Some(vec![0x1b, b'[', b'B']),
        "left" => return Some(vec![0x1b, b'[', b'D']),
        "right" => return Some(vec![0x1b, b'[', b'C']),
        "esc" | "escape" => return Some(vec![0x1b]),
        "space" => return Some(vec![b' ']),
        _ => {}
    }

    // ── Raw escape fallback ──
    // Interpret the string as a Rust-style escape literal (e.g., "\x1b[1;5C").
    // We unescape common sequences.
    parse_raw_escape(name)
}

/// Parse a `ctrl+<key>` modifier.
fn parse_ctrl(key: &str) -> Option<Vec<u8>> {
    // ctrl + arrow keys
    match key {
        "left" => return Some(vec![0x1b, b'[', b'1', b';', b'5', b'D']),
        "right" => return Some(vec![0x1b, b'[', b'1', b';', b'5', b'C']),
        "up" => return Some(vec![0x1b, b'[', b'1', b';', b'5', b'A']),
        "down" => return Some(vec![0x1b, b'[', b'1', b';', b'5', b'B']),
        _ => {}
    }

    // ctrl + single character
    let ch = key.chars().next()?;
    let byte = if ch.is_ascii_alphabetic() {
        (ch.to_ascii_uppercase() as u8) & 0x1f
    } else {
        match ch {
            '@' => 0x00,
            '[' => 0x1b,
            '\\' => 0x1c,
            ']' => 0x1d,
            '^' => 0x1e,
            '_' => 0x1f,
            '?' => 0x7f,
            _ => return None,
        }
    };
    Some(vec![byte])
}

/// Parse an `alt+<key>` modifier.
fn parse_alt(key: &str) -> Option<Vec<u8>> {
    let ch = key.chars().next()?;
    Some(vec![0x1b, ch as u8])
}

/// Parse a `shift+<key>` modifier.
fn parse_shift(key: &str) -> Option<Vec<u8>> {
    match key {
        "left" => Some(vec![0x1b, b'[', b'1', b';', b'2', b'D']),
        "right" => Some(vec![0x1b, b'[', b'1', b';', b'2', b'C']),
        "up" => Some(vec![0x1b, b'[', b'1', b';', b'2', b'A']),
        "down" => Some(vec![0x1b, b'[', b'1', b';', b'2', b'B']),
        "tab" => Some(vec![0x1b, b'[', b'Z']),
        _ => None,
    }
}

/// Generate the CSI escape sequence for a function key.
fn fn_key_bytes(n: u8) -> Vec<u8> {
    let code = match n {
        1 => "11",
        2 => "12",
        3 => "13",
        4 => "14",
        5 => "15",
        6 => "17",
        7 => "18",
        8 => "19",
        9 => "20",
        10 => "21",
        11 => "23",
        12 => "24",
        _ => return Vec::new(),
    };
    let mut v = vec![0x1b, b'['];
    v.extend_from_slice(code.as_bytes());
    v.push(b'~');
    v
}

/// Try to interpret a string as a raw escape sequence.
/// Handles `\x1b`, `\0c`, `\n`, `\r`, `\t`, `\\`, etc.
fn parse_raw_escape(s: &str) -> Option<Vec<u8>> {
    let mut result = Vec::new();
    let mut chars = s.chars();

    while let Some(ch) = chars.next() {
        if ch == '\\' {
            let next = chars.next()?;
            match next {
                'x' | 'X' => {
                    // Hex escape: \x1b, \x0c, etc.
                    let h1 = chars.next()?;
                    let h2 = chars.next()?;
                    let byte = u8::from_str_radix(&format!("{}{}", h1, h2), 16).ok()?;
                    result.push(byte);
                }
                'n' => result.push(b'\n'),
                'r' => result.push(b'\r'),
                't' => result.push(b'\t'),
                '0' => result.push(0x00),
                '\\' => result.push(b'\\'),
                _ => {
                    // Unknown escape — bail out
                    return None;
                }
            }
        } else if ch.is_ascii() {
            result.push(ch as u8);
        } else {
            return None;
        }
    }

    if result.is_empty() {
        None
    } else {
        Some(result)
    }
}

/// Resolve a `KeybindingsConfig` into a vector of `(Vec<u8>, Action)` bindings.
///
/// For each field in the config, this function:
/// 1. Tries to parse the value as a human-readable key name (e.g., "ctrl+right").
/// 2. Falls back to treating it as a raw escape sequence (e.g., "\x1b[1;5C").
/// 3. Skips the binding if neither works.
pub fn resolve_keybindings(config: &KeybindingsConfig) -> Vec<Binding> {
    let mut bindings = Vec::new();

    let try_bind = |name: &str, action: Action, value: &Option<String>, out: &mut Vec<Binding>| {
        if let Some(ref val) = value {
            if let Some(bytes) = parse_key_name(val) {
                out.push(Binding { bytes, action });
            } else {
                tracing::warn!(
                    key = %val, action = %name,
                    "Failed to parse keybinding — ignoring"
                );
            }
        }
    };

    try_bind(
        "next_command",
        Action::NextCommand,
        &config.next_command,
        &mut bindings,
    );
    try_bind(
        "prev_command",
        Action::PrevCommand,
        &config.prev_command,
        &mut bindings,
    );
    try_bind(
        "toggle_log",
        Action::ToggleLog,
        &config.toggle_log,
        &mut bindings,
    );
    try_bind(
        "spawn_command",
        Action::SpawnCommand,
        &config.spawn_command,
        &mut bindings,
    );
    try_bind(
        "show_help",
        Action::ShowHelp,
        &config.show_help,
        &mut bindings,
    );
    try_bind(
        "kill_command",
        Action::KillCommand,
        &config.kill_command,
        &mut bindings,
    );
    try_bind(
        "toggle_pause",
        Action::TogglePause,
        &config.toggle_pause,
        &mut bindings,
    );
    try_bind("quit", Action::Quit, &config.quit, &mut bindings);

    bindings
}

/// Check if an escape buffer matches any binding.
/// Returns `(Some(action), false)` on exact match, `(None, true)` on partial match,
/// `(None, false)` if no bindings could ever match.
pub fn check_bindings<'a>(buf: &'a [u8], bindings: &'a [Binding]) -> (Option<&'a Action>, bool) {
    let mut is_partial = false;
    for binding in bindings {
        if buf.len() >= binding.bytes.len() && buf[..binding.bytes.len()] == binding.bytes[..] {
            return (Some(&binding.action), false);
        }
        if binding.bytes.len() > buf.len() && binding.bytes[..buf.len()] == buf[..] {
            is_partial = true;
        }
    }
    (None, is_partial)
}

/// Format a readable key name from a raw byte sequence (best-effort).
/// Used for displaying keybindings in the help overlay.
pub fn format_key(bytes: &[u8]) -> String {
    match bytes {
        // Function keys (CSI sequences)
        [0x1b, b'[', b'1', b'1', b'~'] => "F1".into(),
        [0x1b, b'[', b'1', b'2', b'~'] => "F2".into(),
        [0x1b, b'[', b'1', b'3', b'~'] => "F3".into(),
        [0x1b, b'[', b'1', b'4', b'~'] => "F4".into(),
        [0x1b, b'[', b'1', b'5', b'~'] => "F5".into(),
        [0x1b, b'[', b'1', b'7', b'~'] => "F6".into(),
        [0x1b, b'[', b'1', b'8', b'~'] => "F7".into(),
        [0x1b, b'[', b'1', b'9', b'~'] => "F8".into(),
        [0x1b, b'[', b'2', b'0', b'~'] => "F9".into(),
        [0x1b, b'[', b'2', b'1', b'~'] => "F10".into(),
        [0x1b, b'[', b'2', b'3', b'~'] => "F11".into(),
        [0x1b, b'[', b'2', b'4', b'~'] => "F12".into(),
        // Modified arrow keys
        [0x1b, b'[', b'1', b';', b'5', b'D'] => "Ctrl+Left".into(),
        [0x1b, b'[', b'1', b';', b'5', b'C'] => "Ctrl+Right".into(),
        [0x1b, b'[', b'1', b';', b'5', b'A'] => "Ctrl+Up".into(),
        [0x1b, b'[', b'1', b';', b'5', b'B'] => "Ctrl+Down".into(),
        [0x1b, b'[', b'1', b';', b'2', b'D'] => "Shift+Left".into(),
        [0x1b, b'[', b'1', b';', b'2', b'C'] => "Shift+Right".into(),
        [0x1b, b'[', b'1', b';', b'2', b'A'] => "Shift+Up".into(),
        [0x1b, b'[', b'1', b';', b'2', b'B'] => "Shift+Down".into(),
        [0x1b, b'[', b'Z'] => "Shift+Tab".into(),
        // Arrow keys
        [0x1b, b'[', b'A'] => "Up".into(),
        [0x1b, b'[', b'B'] => "Down".into(),
        [0x1b, b'[', b'C'] => "Right".into(),
        [0x1b, b'[', b'D'] => "Left".into(),
        // Editing keys
        [0x1b, b'[', b'H'] => "Home".into(),
        [0x1b, b'[', b'F'] => "End".into(),
        [0x1b, b'[', b'3', b'~'] => "Delete".into(),
        [0x1b, b'[', b'2', b'~'] => "Insert".into(),
        [0x1b, b'[', b'5', b'~'] => "PageUp".into(),
        [0x1b, b'[', b'6', b'~'] => "PageDown".into(),
        // Single-byte keys
        [0x1b] => "Esc".into(),
        [0x0d] => "Enter".into(),
        [0x09] => "Tab".into(),
        [0x7f] => "Backspace".into(),
        [0x00] => "Ctrl+@".into(),
        [b] if (0x01..=0x1a).contains(b) => {
            format!("Ctrl+{}", ((*b + 0x40) as char))
        }
        // Alt+key sequences
        [0x1b, ch] => format!("Alt+{}", *ch as char),
        _ => {
            // Fallback: show as hex
            let hex: Vec<String> = bytes.iter().map(|b| format!("\\x{:02x}", b)).collect();
            hex.join("")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ctrl_left() {
        assert_eq!(
            parse_key_name("ctrl+left"),
            Some(vec![0x1b, b'[', b'1', b';', b'5', b'D'])
        );
    }

    #[test]
    fn test_parse_ctrl_right() {
        assert_eq!(
            parse_key_name("ctrl+right"),
            Some(vec![0x1b, b'[', b'1', b';', b'5', b'C'])
        );
    }

    #[test]
    fn test_parse_ctrl_l() {
        assert_eq!(parse_key_name("ctrl+l"), Some(vec![0x0c]));
    }

    #[test]
    fn test_parse_ctrl_h() {
        assert_eq!(parse_key_name("ctrl+h"), Some(vec![0x08]));
    }

    #[test]
    fn test_parse_f12() {
        assert_eq!(
            parse_key_name("f12"),
            Some(vec![0x1b, b'[', b'2', b'4', b'~'])
        );
    }

    #[test]
    fn test_parse_enter() {
        assert_eq!(parse_key_name("enter"), Some(vec![0x0d]));
    }

    #[test]
    fn test_parse_raw_fallback() {
        assert_eq!(
            parse_key_name("\\x1b[1;5C"),
            Some(vec![0x1b, b'[', b'1', b';', b'5', b'C'])
        );
    }

    #[test]
    fn test_format_keys() {
        assert_eq!(
            format_key(&[0x1b, b'[', b'1', b';', b'5', b'D']),
            "Ctrl+Left"
        );
        assert_eq!(format_key(&[0x0c]), "Ctrl+L");
        assert_eq!(format_key(&[0x1b]), "Esc");
        assert_eq!(format_key(&[0x1b, b'[', b'2', b'4', b'~']), "F12");
        assert_eq!(format_key(&[0x1b, b'[', b'1', b'1', b'~']), "F1");
        assert_eq!(format_key(&[0x1b, b'[', b'Z']), "Shift+Tab");
        assert_eq!(format_key(&[0x1b, b'a']), "Alt+a");
    }
}
