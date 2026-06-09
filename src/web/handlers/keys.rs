#![cfg(feature = "vrw")]
#![allow(dead_code, unused_imports)]
use axum::{
    extract::{Path, State},
    Json,
};
use serde_json::Value;

use crate::web::state::AppState;

pub async fn send_keys(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let keys = body.get("keys").and_then(|v| v.as_str()).unwrap_or("");

    if keys.is_empty() {
        return Json(serde_json::json!({
            "status": "error",
            "data": null,
            "error": "Missing 'keys' field"
        }));
    }

    match state.manager.send_keys(&id, keys).await {
        Ok(_) => Json(serde_json::json!({
            "status": "ok",
            "data": { "id": id, "keys_sent": keys },
            "error": null
        })),
        Err(e) => Json(serde_json::json!({
            "status": "error",
            "data": null,
            "error": e.to_string()
        })),
    }
}

/// POST /api/commands/:id/mouse
///
/// Forward a mouse event to the child process's PTY.
/// The event is translated to the appropriate escape sequence based on
/// the current mouse protocol mode (SGR ?1006, or legacy encoding).
///
/// Request body:
///   `event`:  "down" | "up" | "move" | "wheel_up" | "wheel_down"
///   `button`: 0=left, 1=middle, 2=right (for down/up events)
///   `x`:      column (1-based)
///   `y`:      row (1-based)
///   `ctrl`:   boolean (Shift/Ctrl modifiers — currently unused by most apps)
///
/// If the child has not enabled mouse tracking, the event is silently discarded.
pub async fn send_mouse(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let event = body.get("event").and_then(|v| v.as_str()).unwrap_or("");
    let button = body.get("button").and_then(|v| v.as_u64()).unwrap_or(0) as u8;
    let x = body.get("x").and_then(|v| v.as_u64()).unwrap_or(1) as u16;
    let y = body.get("y").and_then(|v| v.as_u64()).unwrap_or(1) as u16;

    if event.is_empty() {
        return Json(serde_json::json!({
            "status": "error",
            "data": null,
            "error": "Missing 'event' field"
        }));
    }

    match state.manager.get(&id) {
        Some(handle) => {
            // Only forward if the child has enabled mouse tracking
            if !handle.mouse_tracking_enabled().await {
                return Json(serde_json::json!({
                    "status": "ok",
                    "data": { "id": id, "forwarded": false },
                    "error": null
                }));
            }

            let sgr = handle.mouse_sgr_enabled().await;
            let seq = encode_mouse_event(event, button, x, y, sgr);

            match handle.send_bytes(seq.into_bytes()).await {
                Ok(_) => Json(serde_json::json!({
                    "status": "ok",
                    "data": { "id": id, "forwarded": true },
                    "error": null
                })),
                Err(e) => Json(serde_json::json!({
                    "status": "error",
                    "data": null,
                    "error": e.to_string()
                })),
            }
        }
        None => Json(serde_json::json!({
            "status": "error",
            "data": null,
            "error": format!("Command {} not found", id)
        })),
    }
}

/// Encode a mouse event as an escape sequence.
///
/// SGR mode (?1006): ESC [ <Cb> ; <Cx> ; <Cy> M/m
///   where Cb = button + 32, M = press/move, m = release
///
/// Legacy mode: ESC [ <Cb> <Cx+32> <Cy+32>
///   where Cb = button + 32
fn encode_mouse_event(event: &str, button: u8, x: u16, y: u16, sgr: bool) -> String {
    let cb = match event {
        "down" => 32 + button,      // press: 32=left, 33=middle, 34=right
        "up" => 32 + 3,             // release always uses button 3
        "move" => 32 + 32 + button, // 64 + button (motion while dragging)
        "wheel_up" => 32 + 64,      // 64 = wheel up
        "wheel_down" => 32 + 65,    // 65 = wheel down
        _ => return String::new(),
    };

    let terminator = if event == "up" { 'm' } else { 'M' };

    if sgr {
        // SGR extended format: CSI Cb ; Cx ; Cy M/m
        format!("\x1b[{};{};{}{}", cb, x, y, terminator)
    } else {
        // Legacy format: CSI Cb Cx Cy  (values > 95 need special encoding)
        let cx = if x > 95 { 95 } else { x as u8 };
        let cy = if y > 95 { 95 } else { y as u8 };
        let cx_enc = if cx == 0 { 32 } else { cx + 31 };
        let cy_enc = if cy == 0 { 32 } else { cy + 31 };
        format!("\x1b[{}{}{}", cb, cx_enc as char, cy_enc as char)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_mouse_down_left_sgr() {
        let seq = encode_mouse_event("down", 0, 10, 20, true);
        assert_eq!(seq, "\x1b[32;10;20M");
    }

    #[test]
    fn test_encode_mouse_down_right_sgr() {
        let seq = encode_mouse_event("down", 2, 5, 3, true);
        assert_eq!(seq, "\x1b[34;5;3M");
    }

    #[test]
    fn test_encode_mouse_up_sgr() {
        let seq = encode_mouse_event("up", 0, 10, 20, true);
        assert_eq!(seq, "\x1b[35;10;20m");
    }

    #[test]
    fn test_encode_mouse_move_sgr() {
        let seq = encode_mouse_event("move", 0, 50, 30, true);
        assert_eq!(seq, "\x1b[64;50;30M");
    }

    #[test]
    fn test_encode_mouse_wheel_up_sgr() {
        let seq = encode_mouse_event("wheel_up", 0, 10, 20, true);
        assert_eq!(seq, "\x1b[96;10;20M");
    }

    #[test]
    fn test_encode_mouse_wheel_down_sgr() {
        let seq = encode_mouse_event("wheel_down", 0, 10, 20, true);
        assert_eq!(seq, "\x1b[97;10;20M");
    }

    #[test]
    fn test_encode_mouse_down_left_legacy() {
        let seq = encode_mouse_event("down", 0, 10, 20, false);
        assert!(seq.starts_with("\x1b[32"));
        // Legacy format doesn't include M/m terminator — just raw CSI params
        assert!(!seq.contains(';')); // SGR uses semicolons, legacy doesn't
    }

    #[test]
    fn test_encode_mouse_down_middle_legacy() {
        let seq = encode_mouse_event("down", 1, 10, 20, false);
        assert!(seq.starts_with("\x1b[33"));
    }

    #[test]
    fn test_encode_mouse_unknown_event() {
        let seq = encode_mouse_event("unknown", 0, 10, 20, true);
        assert!(seq.is_empty());
    }

    #[test]
    fn test_encode_mouse_legacy_clamp_large_values() {
        let seq = encode_mouse_event("down", 0, 200, 300, false);
        // Values > 95 should be clamped
        assert!(seq.starts_with("\x1b[32"));
    }
}
