//! Mouse event types, parsing, selection, and clipboard support.
//!
//! Extracted from display.rs for maintainability. Contains mouse button/event
//! enums, SGR and legacy mouse sequence parsing, visual selection highlight
//! rendering, clipboard copy via OSC 52, and the base64 encoder.

use std::sync::Arc;

use crate::process::manager::CommandManager;

// ── Mouse event types for clipboard selection (#15) ──
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum MouseButton {
    Left,
    Middle,
    Right,
    WheelUp,
    WheelDown,
}
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum MouseEventType {
    Press,
    Release,
    #[allow(dead_code)]
    Motion,
}
#[derive(Debug, Clone)]
pub(crate) struct MouseEvent {
    pub(crate) button: MouseButton,
    pub(crate) event_type: MouseEventType,
    pub(crate) x: u16,
    pub(crate) y: u16,
}
// ── End mouse event types ──

/// Try to parse a mouse event from the escape buffer.
/// Returns Some(MouseEvent) if the buffer contains a complete mouse sequence,
/// None otherwise.  Supports both legacy (`ESC [ M Cb Cr Cc`) and
/// SGR (`ESC [ < Cb ; Cx ; Cy [Mm]`) encodings.
/// Also detects mouse wheel events (SGR cb=64/65, legacy cb=32/33 without motion).
pub(crate) fn try_parse_mouse_event(buf: &[u8]) -> Option<MouseEvent> {
    // SGR encoding: ESC [ < Cb ; Cx ; Cy M (press/drag) or m (release)
    if buf.len() >= 8 && buf.starts_with(b"\x1b[<") {
        let last = *buf.last()?;
        if last != b'M' && last != b'm' {
            return None;
        }
        let is_release = last == b'm';
        let inner = &buf[3..buf.len() - 1];
        let parts: Vec<&[u8]> = inner.splitn(3, |&b| b == b';').collect();
        if parts.len() != 3 {
            return None;
        }
        let cb: u8 = std::str::from_utf8(parts[0]).ok()?.parse().ok()?;
        let cx: u16 = std::str::from_utf8(parts[1]).ok()?.parse().ok()?;
        let cy: u16 = std::str::from_utf8(parts[2]).ok()?.parse().ok()?;
        let is_motion = (cb & 0x20) != 0;
        let button = match cb & 3 {
            0 => MouseButton::Left,
            1 => MouseButton::Middle,
            2 => MouseButton::Right,
            _ => MouseButton::Left,
        };
        // Check for wheel events (SGR encoding uses cb values 64-67)
        let (button, event_type) = if (64..=67).contains(&cb) {
            let wheel = if cb & 1 != 0 {
                MouseButton::WheelDown
            } else {
                MouseButton::WheelUp
            };
            (wheel, MouseEventType::Press)
        } else if is_motion {
            (button, MouseEventType::Motion)
        } else {
            let et = if is_release {
                MouseEventType::Release
            } else {
                MouseEventType::Press
            };
            (button, et)
        };
        return Some(MouseEvent {
            button,
            event_type,
            x: cx,
            y: cy,
        });
    }
    // Legacy encoding: ESC [ M Cb Cx+32 Cy+32
    if buf.len() >= 6 && buf.starts_with(b"\x1b[M") {
        let cb = buf[3];
        let cx = (buf[4].saturating_sub(32)) as u16;
        let cy = (buf[5].saturating_sub(32)) as u16;
        let is_motion = (cb & 0x20) != 0;
        let is_release = !is_motion && ((cb & 0x40) != 0 || (cb & 0x03) == 0x03);
        // Check for wheel events (legacy: cb & 0x43 gives 32/33 for wheel up/down)
        let (button, event_type) = if !is_release && !is_motion && (cb & 0x40) != 0 {
            // Bit 6 set without motion or release means wheel (legacy encoding)
            let wheel = if (cb & 0x01) != 0 {
                MouseButton::WheelDown
            } else {
                MouseButton::WheelUp
            };
            (wheel, MouseEventType::Press)
        } else if is_motion {
            let btn = match cb & 3 {
                0 => MouseButton::Left,
                1 => MouseButton::Middle,
                2 => MouseButton::Right,
                _ => MouseButton::Left,
            };
            (btn, MouseEventType::Motion)
        } else {
            let btn = match cb & 3 {
                0 => MouseButton::Left,
                1 => MouseButton::Middle,
                2 => MouseButton::Right,
                _ => MouseButton::Left,
            };
            let et = if is_release {
                MouseEventType::Release
            } else {
                MouseEventType::Press
            };
            (btn, et)
        };
        return Some(MouseEvent {
            button,
            event_type,
            x: cx,
            y: cy,
        });
    }
    None
}

/// Render a visual selection highlight over the VTTY display.
/// Draws a reverse-video rectangle from start to end coordinates.
pub(crate) fn render_selection_highlight(start: (u16, u16), end: (u16, u16), tab_offset: u16) {
    use std::io::Write;
    let mut stdout = std::io::stdout();
    let (min_row, max_row) = if start.0 <= end.0 {
        (start.0, end.0)
    } else {
        (end.0, start.0)
    };
    let (min_col, max_col) = if start.1 <= end.1 {
        (start.1, end.1)
    } else {
        (end.1, start.1)
    };
    for row in min_row..=max_row {
        let screen_row = row + tab_offset;
        let col_start = if row == min_row { min_col } else { 0 };
        let col_end = if row == max_row { max_col } else { u16::MAX };
        let _ = write!(stdout, "\x1b[{};{}H", screen_row + 1, col_start + 1);
        let _ = write!(stdout, "\x1b[7m"); // reverse video
        let _ = write!(stdout, "\x1b[{};{}H", screen_row + 1, col_start + 1);
        // We can't know the exact cell content here, so we mark positions
        // The visual effect is provided by the reverse video styling
        if col_end == u16::MAX {
            let _ = write!(stdout, "\x1b[0K"); // clear to end of line (shows reverse bg)
        } else {
            let len = (col_end.saturating_sub(col_start) + 1) as usize;
            let _ = write!(stdout, "{}", " ".repeat(len));
        }
        let _ = write!(stdout, "\x1b[0m"); // reset
    }
    let _ = stdout.flush();
}

/// Extract text from the VTTY buffer for the selected region and copy to clipboard.
/// Uses OSC 52 escape sequence to set the clipboard (works in xterm, kitty, etc.).
pub(crate) fn copy_selection_to_clipboard(
    manager: &Arc<CommandManager>,
    active_id: &Option<String>,
    start: (u16, u16),
    end: (u16, u16),
    _tab_offset: u16,
) {
    use std::io::Write;
    let commands = manager.list();
    let target_id = active_id
        .as_ref()
        .or_else(|| commands.first().map(|(id, _, _, _, _)| id));

    if let Some(id) = target_id {
        if let Some(handle) = manager.get(id) {
            let buf = handle.vtty_snapshot_blocking();
            let (min_row, max_row) = if start.0 <= end.0 {
                (start.0, end.0)
            } else {
                (end.0, start.0)
            };
            let (min_col, max_col) = if start.1 <= end.1 {
                (start.1, end.1)
            } else {
                (end.1, start.1)
            };
            let total_lines = buf.total_lines();
            let viewport_start = total_lines.saturating_sub(buf.height);

            let mut selected_text = String::new();
            for row in min_row..=max_row {
                let line_idx = viewport_start.saturating_add(row as usize);
                if let Some(line) = buf.get_line(line_idx) {
                    let col_start = if row == min_row { min_col as usize } else { 0 };
                    let col_end = if row == max_row {
                        max_col as usize
                    } else {
                        line.len()
                    };
                    for cell in line
                        .iter()
                        .skip(col_start)
                        .take(col_end.saturating_sub(col_start))
                    {
                        if cell.width > 0 {
                            selected_text.push(cell.ch);
                        }
                    }
                    if row < max_row {
                        selected_text.push('\n');
                    }
                }
            }

            if !selected_text.is_empty() {
                // Use OSC 52 to copy to clipboard
                // Format: ESC ] 52 ; c ; <base64> BEL
                let encoded = base64_encode(&selected_text);
                let mut stdout = std::io::stdout();
                let _ = write!(stdout, "\x1b]52;c;{}\x07", encoded);
                let _ = stdout.flush();
                tracing::debug!(
                    len = selected_text.len(),
                    "Copied selection to clipboard via OSC 52"
                );
            }
        }
    }
}

/// Simple base64 encoder for clipboard content (avoids adding a dependency).
pub(crate) fn base64_encode(input: &str) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes = input.as_bytes();
    let mut result = String::new();
    let mut i = 0;
    while i + 3 <= bytes.len() {
        let n = (bytes[i] as u32) << 16 | (bytes[i + 1] as u32) << 8 | (bytes[i + 2] as u32);
        result.push(CHARS[((n >> 18) & 63) as usize] as char);
        result.push(CHARS[((n >> 12) & 63) as usize] as char);
        result.push(CHARS[((n >> 6) & 63) as usize] as char);
        result.push(CHARS[(n & 63) as usize] as char);
        i += 3;
    }
    if i + 2 <= bytes.len() {
        let n = (bytes[i] as u32) << 16 | (bytes[i + 1] as u32) << 8;
        result.push(CHARS[((n >> 18) & 63) as usize] as char);
        result.push(CHARS[((n >> 12) & 63) as usize] as char);
        result.push(CHARS[((n >> 6) & 63) as usize] as char);
        result.push('=');
    } else if i < bytes.len() {
        let n = (bytes[i] as u32) << 16;
        result.push(CHARS[((n >> 18) & 63) as usize] as char);
        result.push(CHARS[((n >> 12) & 63) as usize] as char);
        result.push('=');
        result.push('=');
    }
    result
}
