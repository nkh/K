use chrono::Local;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

const MEMORY_BUFFER_CAPACITY: usize = 2048;
const ID_WIDTH: usize = 8;
const CMD_WIDTH: usize = 20;

/// Shared in-memory log buffer type used by both the web UI handler
/// and the terminal display loop.
pub type SharedLogBuffer = Arc<Mutex<Vec<String>>>;

pub struct CommandLogger {
    enabled: bool,
    file: Option<Mutex<std::fs::File>>,
    /// In-memory ring buffer of recent log entries.
    /// This allows the web UI log viewer and terminal overlay to show
    /// entries even when no log file is configured (e.g. --log without --log-file).
    memory_buffer: SharedLogBuffer,
    /// Broadcast channel for streaming log entries to WebSocket subscribers
    /// and the non-display terminal event loop.
    log_tx: broadcast::Sender<String>,
    /// When true, ANSI color escape codes are included in terminal output.
    color_terminal_log: bool,
}

impl CommandLogger {
    pub fn new(
        enabled: bool,
        file_path: Option<&str>,
        _binary_name: &str,
        color_terminal_log: bool,
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
    /// Used by the terminal display loop for the log overlay.
    pub fn memory_buffer_arc(&self) -> SharedLogBuffer {
        Arc::clone(&self.memory_buffer)
    }

    /// Read all entries from the in-memory ring buffer.
    /// Returns entries in chronological order (oldest first).
    pub fn read_memory_buffer(&self) -> Vec<String> {
        let buf = self.memory_buffer.lock().unwrap_or_else(|e| e.into_inner());
        buf.iter().cloned().collect()
    }

    /// Extract the command id from a details string.
    /// Looks for `id=<value>` where value is the first UUID-like token.
    fn extract_id(details: &str) -> &str {
        for part in details.split(' ') {
            if let Some(id) = part.strip_prefix("id=") {
                // Take only up to 8 chars, or until a comma/non-hex
                let end = id.len().min(ID_WIDTH);
                return &id[..end];
            }
        }
        ""
    }

    /// Extract the command name from a details string.
    /// Looks for `cmd=<value>` or `name=<value>`.
    fn extract_cmd_name(details: &str) -> &str {
        for part in details.split(' ') {
            if let Some(val) = part.strip_prefix("cmd=") {
                return val;
            }
            if let Some(val) = part.strip_prefix("name=") {
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

    /// Pad a string to a fixed width, truncating if longer.
    fn pad_field(s: &str, width: usize) -> String {
        if s.len() >= width {
            s[..width].to_string()
        } else {
            format!("{:width$}", s, width = width)
        }
    }

    /// ANSI color codes for terminal output.
    /// Only used when `color_terminal_log` is true.
    const CLR_RESET: &'static str = "\x1b[0m";
    const CLR_TIMESTAMP: &'static str = "\x1b[90m";     // dark grey
    const CLR_ID: &'static str = "\x1b[32m";           // green
    const CLR_CMD: &'static str = "\x1b[1;37m";        // bright white
    const CLR_EVENT: &'static str = "\x1b[1;37m";      // bright white (event type)
    // Detail field colors
    const CLR_ARG: &'static str = "\x1b[1;37m";        // bright white
    const CLR_CERT: &'static str = "\x1b[34m";         // blue
    const CLR_ENV: &'static str = "\x1b[32m";          // green
    const CLR_SIZE: &'static str = "\x1b[1;33m";       // bright yellow
    const CLR_DIR: &'static str = "\x1b[34m";          // blue
    const CLR_DETAIL_DEFAULT: &'static str = "\x1b[90m"; // dark grey (for other fields)

    /// Color a single detail token based on its key prefix.
    /// Returns (colored_token, is_id_or_cmd).
    fn color_detail_token(token: &str) -> (String, bool) {
        if let Some(_) = token.strip_prefix("id=") {
            // Skip id in details — already shown as a separate field
            return (String::new(), true);
        }
        if let Some(_) = token.strip_prefix("cmd=") {
            // Skip cmd in details — already shown as a separate field
            return (String::new(), true);
        }
        if let Some(_) = token.strip_prefix("args=") {
            return (format!("{}{}{}", Self::CLR_ARG, token, Self::CLR_RESET), false);
        }
        if let Some(_) = token.strip_prefix("cert=") {
            return (format!("{}{}{}", Self::CLR_CERT, token, Self::CLR_RESET), false);
        }
        if let Some(_) = token.strip_prefix("env=") {
            return (format!("{}{}{}", Self::CLR_ENV, token, Self::CLR_RESET), false);
        }
        if let Some(_) = token.strip_prefix("size=") {
            return (format!("{}{}{}", Self::CLR_SIZE, token, Self::CLR_RESET), false);
        }
        if let Some(_) = token.strip_prefix("dir=") {
            return (format!("{}{}{}", Self::CLR_DIR, token, Self::CLR_RESET), false);
        }
        // Default color for other tokens
        return (format!("{}{}{}", Self::CLR_DETAIL_DEFAULT, token, Self::CLR_RESET), false);
    }

    /// Build a colored details string, skipping id= and cmd= tokens.
    fn color_details(details: &str) -> String {
        let tokens: Vec<&str> = details.split(' ').collect();
        let mut colored = String::new();
        let mut first = true;
        for token in tokens {
            let (ctoken, skip) = Self::color_detail_token(token);
            if skip {
                continue;
            }
            if ctoken.is_empty() {
                continue;
            }
            if !first {
                colored.push(' ');
            }
            first = false;
            colored.push_str(&ctoken);
        }
        colored
    }

    /// Strip id= and cmd= tokens from a details string (for terminal output
    /// where they're redundant).
    fn strip_id_cmd(details: &str) -> String {
        let tokens: Vec<&str> = details.split(' ').filter(|t| {
            !(t.starts_with("id=") || t.starts_with("cmd=") || t.starts_with("name="))
        }).collect();
        tokens.join(" ")
    }

    pub fn log(&self, event_type: &str, details: &str) {
        let timestamp = Self::format_timestamp();
        let id_short = Self::extract_id(details);
        let id_padded = Self::pad_field(id_short, ID_WIDTH);
        let cmd_name = Self::extract_cmd_name(details);
        let cmd_padded = Self::pad_field(cmd_name, CMD_WIDTH);

        // Terminal line (space-separated fields, optional color)
        let term_line = if self.color_terminal_log {
            let colored_details = Self::color_details(details);
            format!(
                "{ts}{clr_ts} {id}{clr_id} {cmd}{clr_cmd} {evt}{clr_evt}: {details}{clr_reset}\n",
                ts = timestamp,
                clr_ts = Self::CLR_TIMESTAMP,
                id = id_padded,
                clr_id = Self::CLR_ID,
                cmd = cmd_padded,
                clr_cmd = Self::CLR_CMD,
                evt = event_type,
                clr_evt = Self::CLR_EVENT,
                details = colored_details,
                clr_reset = Self::CLR_RESET,
            )
        } else {
            // Plain: strip id and cmd from details to avoid repetition
            let clean_details = Self::strip_id_cmd(details);
            format!(
                "{} {} {} {}: {}\n",
                timestamp, id_padded, cmd_padded, event_type, clean_details
            )
        };

        let term_trimmed = term_line.trim_end().to_string();

        // File line (tab-separated fields, no color, no padding)
        let file_line = format!(
            "{}\t{}\t{}\t{}\t{}\n",
            timestamp, id_short, cmd_name, event_type, details
        );

        // Always populate the in-memory ring buffer and broadcast,
        // regardless of whether file logging is enabled.  This ensures
        // the non-display event loop, web UI log viewer, and --display
        // log overlay can show events without requiring --log.
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
