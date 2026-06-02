use chrono::Local;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

const MEMORY_BUFFER_CAPACITY: usize = 2048;
const BINARY_NAME_WIDTH: usize = 4; // "vrw " / "vrc "
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
    /// The binary name (e.g. "vrw" or "vrc") included in every log line.
    binary_name: String,
    /// When true, ANSI color escape codes are included in terminal output.
    color_always: bool,
}

impl CommandLogger {
    pub fn new(
        enabled: bool,
        file_path: Option<&str>,
        binary_name: &str,
        color_always: bool,
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
            binary_name: binary_name.to_string(),
            color_always,
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
    /// Only used when `color_always` is true.
    const COLOR_RESET: &'static str = "\x1b[0m";
    const COLOR_TIMESTAMP: &'static str = "\x1b[2m";  // dim
    const COLOR_BINARY: &'static str = "\x1b[36m";   // cyan
    const COLOR_ID: &'static str = "\x1b[33m";       // yellow
    const COLOR_CMD: &'static str = "\x1b[32m";      // green
    const COLOR_EVENT: &'static str = "\x1b[1;34m";   // bold blue

    pub fn log(&self, event_type: &str, details: &str) {
        let timestamp = Self::format_timestamp();
        let bin_padded = Self::pad_field(&self.binary_name, BINARY_NAME_WIDTH);
        let id_short = Self::extract_id(details);
        let id_padded = Self::pad_field(id_short, ID_WIDTH);
        let cmd_name = Self::extract_cmd_name(details);
        let cmd_padded = Self::pad_field(cmd_name, CMD_WIDTH);

        // Terminal line (space-separated fields, optional color)
        let term_line = if self.color_always {
            format!(
                "{ts}{clr_ts}  {bin}{clr_bin}  {id}{clr_id}  {cmd}{clr_cmd}  {evt}{clr_evt}{details}{clr_reset}\n",
                ts = timestamp,
                clr_ts = Self::COLOR_TIMESTAMP,
                bin = bin_padded,
                clr_bin = Self::COLOR_BINARY,
                id = id_padded,
                clr_id = Self::COLOR_ID,
                cmd = cmd_padded,
                clr_cmd = Self::COLOR_CMD,
                evt = event_type,
                clr_evt = Self::COLOR_EVENT,
                details = details,
                clr_reset = Self::COLOR_RESET,
            )
        } else {
            format!(
                "{}  {}  {}  {}  {}: {}\n",
                timestamp, bin_padded, id_padded, cmd_padded, event_type, details
            )
        };

        let term_trimmed = term_line.trim_end().to_string();

        // File line (tab-separated fields, no color)
        let file_line = format!(
            "{}\t{}\t{}\t{}\t{}: {}\n",
            timestamp, self.binary_name, id_short, cmd_name, event_type, details
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
            // Use the colored terminal line for stdout
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
