//! Event tracer — writes structured event lines to file descriptor 3.
//!
//! Activated with `-v` / `--show-events` (stackable).  Events are one-line
//! traces of every WebSocket and HTTP interaction the server handles.
//!
//! fd 3 is used so the user can redirect independently:
//!   `vrw -v htop 3>/tmp/events.log`
//!   `vrw -vv htop 3>&1`
//!
//! Verbosity levels:
//!   1  (`-v`)   — timestamp, direction arrow, source, session, msg type, truncated payload
//!   2  (`-vv`)  — + full payload (up to 500 chars), HTTP request body hint
//!   3  (`-vvv`) — + internal detail (broadcast subscriber count, baseline UUID, etc.)
//!
//! Filtering:
//!   `--event-regexp 'vtty_dirty'` — only emit lines matching the regex.
//!   The regex is applied to the full formatted line (including direction/source prefix).

use std::ffi::c_int;
use std::fs::File;
use std::io::Write;
use std::os::unix::io::FromRawFd;
use std::sync::Mutex;

/// ANSI escape helpers (only used when color is enabled).
mod color {
    pub const RESET: &str = "\x1b[0m";
    pub const DIM: &str = "\x1b[2m";
    pub const GREEN: &str = "\x1b[32m";
    pub const CYAN: &str = "\x1b[36m";
    pub const YELLOW: &str = "\x1b[33m";
    pub const GRAY: &str = "\x1b[90m";
}

static TRACER: Mutex<Option<TracerInner>> = Mutex::new(None);

struct TracerInner {
    file: File,
    verbosity: u8,
    color: bool,
    regex: Option<regex::Regex>,
}

/// Initialize the global tracer.  Call once before the server starts.
/// If `verbosity` is 0, no tracing happens (the tracer stays `None`).
pub fn init(verbosity: u8, color: bool, event_regexp: Option<&str>) {
    if verbosity == 0 {
        return;
    }

    // Open fd 3.  Use libc::dup to get a Rust File handle.
    // SAFETY: fd 3 must be valid and open for writing.  If not, dup returns -1
    // and we silently disable tracing (the user didn't redirect fd 3).
    let fd: c_int = 3;
    let duped = unsafe { libc::dup(fd) };
    if duped < 0 {
        // fd 3 not open — tracing disabled.
        return;
    }
    let file = unsafe { File::from_raw_fd(duped) };

    let regex = match event_regexp {
        Some(pat) => match regex::Regex::new(pat) {
            Ok(re) => Some(re),
            Err(e) => {
                eprintln!("warning: --event-regexp invalid: {e}, ignoring");
                None
            }
        },
        None => None,
    };

    let mut guard = TRACER.lock().unwrap();
    *guard = Some(TracerInner {
        file,
        verbosity,
        color,
        regex,
    });
}

/// Direction of the event.
#[derive(Debug, Clone, Copy)]
pub enum Direction {
    /// Server → client (outgoing)
    Send,
    /// Client → server (incoming)
    Recv,
}

/// Source of the event.
#[derive(Debug, Clone, Copy)]
pub enum Source {
    WebSocket,
    Http,
}

/// Trace a WebSocket or HTTP event.
///
/// * `dir` — direction (send or recv)
/// * `source` — ws or http
/// * `session_id` — short identifier (WS connection ID, or HTTP request ID)
/// * `msg_type` — message type string (e.g. "vtty_dirty", "POST")
/// * `payload` — the data/payload (JSON string for WS, body hint for HTTP)
/// * `detail` — optional extra detail shown only at verbosity >= 3
pub fn event(
    dir: Direction,
    source: Source,
    session_id: &str,
    msg_type: &str,
    payload: &str,
    detail: Option<&str>,
) {
    // Fast path: check if tracer is active (immutable borrow to read verbosity).
    {
        let guard = match TRACER.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        let tracer = match guard.as_ref() {
            Some(t) => t,
            None => return,
        };
        if tracer.verbosity == 0 {
            return;
        }
    }

    let now = chrono::Local::now();
    let ts = now.format("%H:%M:%S%.3f");

    let arrow = match dir {
        Direction::Send => "\u{2192}",
        Direction::Recv => "\u{2190}",
    };

    let src_label = match source {
        Source::WebSocket => "ws",
        Source::Http => "http",
    };

    // Build the line and apply formatting (need tracer fields).
    let output = {
        let guard = TRACER.lock().unwrap();
        let tracer = guard.as_ref().unwrap();

        let max_payload_len = if tracer.verbosity == 1 { 80 } else { 500 };
        let truncated = truncate(payload, max_payload_len);

        let mut line = format!("{ts} {arrow} {src_label}:{session_id} {msg_type} {truncated}");

        if let Some(d) = detail {
            if tracer.verbosity >= 3 {
                line.push_str(&format!(" | {d}"));
            }
        }

        // Apply regex filter
        if let Some(ref re) = tracer.regex {
            if !re.is_match(&line) {
                return;
            }
        }

        // Apply colors if enabled
        if tracer.color {
            let arrow_color = match dir {
                Direction::Send => color::GREEN,
                Direction::Recv => color::CYAN,
            };
            let src_color = match source {
                Source::WebSocket => "",
                Source::Http => color::YELLOW,
            };
            format!(
                "{dim}{ts}{reset} {arrow_clr}{arrow}{reset} {src_clr}{src_label}{reset}:{session_id} {msg_type} {truncated}{reset}",
                dim = color::DIM,
                reset = color::RESET,
                arrow_clr = arrow_color,
                src_clr = src_color,
            )
        } else {
            line
        }
    };

    // Write (separate scope to drop guard before write).
    // Actually we need mutable access for writeln, so re-lock.
    if let Ok(mut guard) = TRACER.lock() {
        if let Some(tracer) = guard.as_mut() {
            let _ = writeln!(tracer.file, "{output}");
        }
    }
}

/// Trace an HTTP request/response pair in a single line.
///
/// * `method` — HTTP method (GET, POST, etc.)
/// * `path` — request path
/// * `status` — response status code (0 if request not yet responded)
/// * `body_hint` — short description of request/response body
pub fn http_event(method: &str, path: &str, status: u16, body_hint: &str) {
    let msg_type = format!("{method} {path} {status}");
    event(Direction::Recv, Source::Http, "-", &msg_type, body_hint, None);
}

/// Trace an HTTP response (outgoing).
pub fn http_response(method: &str, path: &str, status: u16, body_hint: &str) {
    let msg_type = format!("{method} {path} {status}");
    event(Direction::Send, Source::Http, "-", &msg_type, body_hint, None);
}

/// Extract the message `type` field from a JSON string, for use as `msg_type`.
pub fn json_msg_type(json: &str) -> &str {
    // Fast path: find "type":"..." or "type": "..."
    if let Some(pos) = json.find("\"type\"") {
        let rest = &json[pos + 6..];
        // skip whitespace and colon
        let rest = rest.trim_start();
        if rest.starts_with(':') {
            let rest = rest[1..].trim_start();
            if rest.starts_with('"') {
                if let Some(end) = rest[1..].find('"') {
                    return &rest[1..end + 1];
                }
            }
        }
    }
    "?"
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        // Break at a char boundary
        let mut end = max;
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        &s[..end]
    }
}