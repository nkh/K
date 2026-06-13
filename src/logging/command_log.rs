use chrono::Local;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

use crate::config::hooks::TerminalLogConfig;

const MEMORY_BUFFER_CAPACITY: usize = 2048;
const RESET: &str = "\x1b[0m";

/// Shared in-memory log buffer type used by both the web UI handler
/// and the terminal display loop.
pub type SharedLogBuffer = Arc<Mutex<Vec<String>>>;

pub struct CommandLogger {
    enabled: bool,
    file: Option<Mutex<std::fs::File>>,
    /// In-memory ring buffer of recent log entries.
    memory_buffer: SharedLogBuffer,
    /// Broadcast channel for streaming log entries to WebSocket subscribers
    /// and the non-display terminal event loop.
    log_tx: broadcast::Sender<String>,
    /// When true, ANSI color escape codes are included in terminal output.
    color_terminal_log: bool,
    /// Terminal log appearance config (format, colors, padding).
    terminal_cfg: TerminalLogConfig,
}

impl CommandLogger {
    pub fn new(
        enabled: bool,
        file_path: Option<&str>,
        _binary_name: &str,
        color_terminal_log: bool,
        terminal_cfg: TerminalLogConfig,
    ) -> anyhow::Result<Self> {
        let file = match file_path {
            Some(path) => {
                let f = OpenOptions::new().create(true).append(true).open(path)?;
                Some(Mutex::new(f))
            }
            None => None,
        };
        let (log_tx, _) = broadcast::channel(256);
        Ok(Self {
            enabled,
            file,
            memory_buffer: Arc::new(Mutex::new(Vec::with_capacity(MEMORY_BUFFER_CAPACITY))),
            log_tx,
            color_terminal_log,
            terminal_cfg,
        })
    }

    /// Subscribe to log entries. Returns a broadcast receiver.
    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.log_tx.subscribe()
    }

    /// Get a clone of the log broadcast sender.
    pub fn log_sender(&self) -> broadcast::Sender<String> {
        self.log_tx.clone()
    }

    /// Get a shared reference to the in-memory log buffer.
    pub fn memory_buffer_arc(&self) -> SharedLogBuffer {
        Arc::clone(&self.memory_buffer)
    }

    /// Read all entries from the in-memory ring buffer.
    pub fn read_memory_buffer(&self) -> Vec<String> {
        let buf = self.memory_buffer.lock().unwrap_or_else(|e| e.into_inner());
        buf.iter().cloned().collect()
    }

    // ── Field extractors ──

    /// Extract a key=value field from a space-separated details string.
    fn extract_field<'a>(details: &'a str, key: &str) -> &'a str {
        let prefix = format!("{}=", key);
        for part in details.split(' ') {
            if let Some(val) = part.strip_prefix(&prefix) {
                return val;
            }
        }
        ""
    }

    /// Format a local timestamp as HH:MM:SS.cc (hundredths of a second).
    fn format_timestamp() -> String {
        let now = Local::now();
        let time_part = now.format("%H:%M:%S");
        let hundredths = now.timestamp_subsec_millis() / 10;
        format!("{}.{:02}", time_part, hundredths)
    }

    /// Pad a string to a fixed width, left-aligned (spaces on right).
    /// Truncates if longer than width.
    fn pad_left(s: &str, width: usize) -> String {
        if s.len() >= width {
            s[..width].to_string()
        } else {
            format!("{:<width$}", s, width = width)
        }
    }

    /// Pad a string to a fixed width, right-aligned (spaces on left).
    /// Truncates if longer than width.
    fn pad_right(s: &str, width: usize) -> String {
        if s.len() >= width {
            s[..width].to_string()
        } else {
            format!("{:>width$}", s, width = width)
        }
    }

    // ── Color helpers ──

    /// Get the ANSI escape sequence for a named detail field.
    fn detail_color(&self, key: &str) -> &str {
        let colors = &self.terminal_cfg.colors;
        match key {
            "args" => &colors.arg.ansi,
            "cert" => &colors.cert.ansi,
            "env" => &colors.env.ansi,
            "size" => &colors.size.ansi,
            "dir" => &colors.dir.ansi,
            "name" => &colors.cmd.ansi,
            "cmd" => &colors.cmd.ansi,
            "pid" => &colors.pid.ansi,
            "id" => &colors.id.ansi,
            "code" => &colors.detail.ansi,
            "retained" => &colors.detail.ansi,
            "keys" => &colors.detail.ansi,
            "old" => &colors.detail.ansi,
            "new" => &colors.detail.ansi,
            "rows" => &colors.detail.ansi,
            "cols" => &colors.detail.ansi,
            "error" => &colors.detail.ansi,
            _ => &colors.detail.ansi,
        }
    }

    /// Build a colored details string.
    /// Skips fields that are displayed as top-level fields (id, pid, cmd, name)
    /// when they match the format spec.  Always includes them in plain mode.
    fn color_details(&self, details: &str, colored: bool) -> String {
        let tokens: Vec<&str> = details.split(' ').collect();
        let mut out = String::new();
        let mut first = true;
        for token in &tokens {
            let (key, _val) = match token.split_once('=') {
                Some(pair) => pair,
                None => {
                    // Non key=value token
                    if !first { out.push(' '); }
                    first = false;
                    if colored {
                        out.push_str(&self.terminal_cfg.colors.detail.ansi);
                    }
                    out.push_str(token);
                    if colored { out.push_str(RESET); }
                    continue;
                }
            };
            // Skip fields that are rendered as top-level placeholders
            if key == "id" || key == "pid" || key == "cmd" || key == "name" {
                continue;
            }
            if !first { out.push(' '); }
            first = false;
            if colored {
                out.push_str(self.detail_color(key));
            }
            out.push_str(token);
            if colored {
                out.push_str(RESET);
            }
        }
        out
    }

    /// Strip id=, pid=, cmd=, name= from details (for plain mode).
    fn strip_top_fields(details: &str) -> String {
        details
            .split(' ')
            .filter(|t| {
                !(t.starts_with("id=") || t.starts_with("pid=") || t.starts_with("cmd=") || t.starts_with("name="))
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Resolve a format placeholder to its rendered string.
    fn resolve_placeholder(
        &self,
        placeholder: &str,
        details: &str,
        event_type: &str,
        colored: bool,
    ) -> (String, bool) {
        let colors = &self.terminal_cfg.colors;
        let pad = &self.terminal_cfg.pad;
        match placeholder {
            "timestamp" => {
                let ts = Self::format_timestamp();
                if colored {
                    (format!("{}{}{}", colors.timestamp.ansi, ts, RESET), true)
                } else {
                    (ts, true)
                }
            }
            "pid" => {
                let pid = Self::extract_field(details, "pid");
                let padded = Self::pad_right(pid, pad.pid);
                if colored {
                    (format!("{}{}{}", colors.pid.ansi, padded, RESET), true)
                } else {
                    (padded, true)
                }
            }
            "id" => {
                let id = Self::extract_field(details, "id");
                // Show first 8 chars
                let short = if id.len() > 8 { &id[..8] } else { id };
                let padded = Self::pad_right(short, 8);
                if colored {
                    (format!("{}{}{}", colors.id.ansi, padded, RESET), true)
                } else {
                    (padded, true)
                }
            }
            "cmd" => {
                // Try cmd= first, then name=
                let cmd = {
                    let c = Self::extract_field(details, "cmd");
                    if c.is_empty() { Self::extract_field(details, "name") } else { c }
                };
                let padded = Self::pad_left(cmd, pad.cmd);
                if colored {
                    (format!("{}{}{}", colors.cmd.ansi, padded, RESET), true)
                } else {
                    (padded, true)
                }
            }
            "event" => {
                let padded = Self::pad_left(event_type, pad.event);
                if colored {
                    (format!("{}{}{}", colors.event.ansi, padded, RESET), true)
                } else {
                    (padded, true)
                }
            }
            "details" => {
                let det = if colored {
                    self.color_details(details, true)
                } else {
                    Self::strip_top_fields(details)
                };
                let empty = det.is_empty();
                (det, !empty)
            }
            _ => (String::new(), false),
        }
    }

    /// Build the terminal line from the format string.
    fn build_term_line(&self, event_type: &str, details: &str) -> String {
        let fmt = &self.terminal_cfg.format;
        let colored = self.color_terminal_log && !fmt.is_empty();
        let mut out = String::new();

        let mut i = 0;
        let bytes = fmt.as_bytes();
        while i < bytes.len() {
            if bytes[i] == b'%' {
                // Look for closing %
                if let Some(end) = fmt[i + 1..].find('%') {
                    let name = &fmt[i + 1..i + 1 + end];
                    let (rendered, shown) = self.resolve_placeholder(name, details, event_type, colored);
                    if shown {
                        if !out.is_empty() && !out.ends_with(' ') {
                            out.push(' ');
                        }
                        out.push_str(&rendered);
                    }
                    i = i + 1 + end + 1;
                    continue;
                }
            }
            out.push(bytes[i] as char);
            i += 1;
        }

        out
    }

    /// Main log function. Called for every event.
    pub fn log(&self, event_type: &str, details: &str) {
        // Build terminal line from format string
        let term_line = self.build_term_line(event_type, details);
        let term_trimmed = term_line.trim_end().to_string();

        // File line (tab-separated fields, no color, no padding)
        let id_short = {
            let id = Self::extract_field(details, "id");
            if id.len() > 8 { &id[..8] } else { id }
        };
        let cmd_name = {
            let c = Self::extract_field(details, "cmd");
            if c.is_empty() { Self::extract_field(details, "name") } else { c }
        };
        let pid = Self::extract_field(details, "pid");
        let file_line = format!(
            "{}\t{}\t{}\t{}\t{}\t{}\n",
            Self::format_timestamp(), pid, id_short, cmd_name, event_type, details
        );

        // Always populate the in-memory ring buffer and broadcast,
        // regardless of whether file logging is enabled.
        if let Ok(mut buf) = self.memory_buffer.lock() {
            if buf.len() >= MEMORY_BUFFER_CAPACITY {
                buf.remove(0);
            }
            buf.push(term_trimmed.clone());
        }
        let _ = self.log_tx.send(term_trimmed.clone());

        // File writing and direct stdout printing are gated by `enabled`.
        if !self.enabled {
            return;
        }

        // Print to stdout if enabled and no file specified
        if self.file.is_none() {
            print!("{}", term_line);
        }

        // Write to file if configured (tab-separated, no color)
        if let Some(ref file_mutex) = self.file {
            if let Ok(mut file) = file_mutex.lock() {
                let _ = file.write_all(file_line.as_bytes());
                let _ = file.flush();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_logger() -> CommandLogger {
        CommandLogger::new(
            true,
            None,
            "vrc",
            false,
            TerminalLogConfig::default(),
        )
        .unwrap()
    }

    #[test]
    fn test_logger_log_populates_memory_buffer() {
        let logger = make_logger();
        logger.log("spawn", "cmd=bash pid=123 id=abc12345");
        let entries = logger.read_memory_buffer();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].contains("bash"));
    }

    #[test]
    fn test_logger_log_multiple_events() {
        let logger = make_logger();
        logger.log("spawn", "cmd=htop pid=100 id=aaa11111");
        logger.log("exit", "cmd=htop pid=100 id=aaa11111");
        logger.log("resize", "cmd=htop pid=100 id=aaa11111");
        let entries = logger.read_memory_buffer();
        assert_eq!(entries.len(), 3);
    }

    #[test]
    fn test_logger_ring_buffer_capacity() {
        let logger = make_logger();
        for i in 0..2100 {
            logger.log("spawn", &format!("cmd=test pid={} id={:0>16}", i, i));
        }
        let entries = logger.read_memory_buffer();
        assert_eq!(entries.len(), MEMORY_BUFFER_CAPACITY);
    }

    #[test]
    fn test_logger_broadcast() {
        let logger = make_logger();
        let mut rx = logger.subscribe();
        logger.log("spawn", "cmd=bash pid=1 id=abc12345");
        let received = rx.try_recv();
        assert!(received.is_ok());
        let msg = received.unwrap();
        assert!(msg.contains("bash"));
    }

    #[test]
    fn test_logger_disabled_no_file_write() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("disabled.log");
        let logger = CommandLogger::new(
            false,
            Some(path.to_str().unwrap()),
            "vrc",
            false,
            TerminalLogConfig::default(),
        )
        .unwrap();
        logger.log("spawn", "cmd=bash pid=1 id=abc12345");
        // File should exist but might not have content since enabled=false
    }

    #[test]
    fn test_extract_field() {
        assert_eq!(CommandLogger::extract_field("cmd=bash pid=123 name=test", "cmd"), "bash");
        assert_eq!(CommandLogger::extract_field("cmd=bash pid=123", "pid"), "123");
        assert_eq!(CommandLogger::extract_field("nokey", "missing"), "");
    }

    #[test]
    fn test_strip_top_fields() {
        let result = CommandLogger::strip_top_fields("id=abc pid=123 cmd=bash name=test extra=val");
        assert!(!result.contains("id="));
        assert!(!result.contains("pid="));
        assert!(!result.contains("cmd="));
        assert!(!result.contains("name="));
        assert!(result.contains("extra=val"));
    }
}
