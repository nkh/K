use std::fs::OpenOptions;
use std::io::Write;
use std::sync::{Arc, Mutex};
use chrono::Utc;
use tokio::sync::broadcast;

const MEMORY_BUFFER_CAPACITY: usize = 2048;

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
    /// Broadcast channel for streaming log entries to WebSocket subscribers.
    log_tx: broadcast::Sender<String>,
}

impl CommandLogger {
    pub fn new(enabled: bool, file_path: Option<&str>) -> anyhow::Result<Self> {
        let file = match file_path {
            Some(path) => {
                let f = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)?;
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

    pub fn log(&self, command: &str, details: &str) {
        if !self.enabled {
            return;
        }
        let timestamp = Utc::now().to_rfc3339();
        let line = format!("[{}] {}: {}\n", timestamp, command, details);
        let trimmed = line.trim_end().to_string();

        // Always print to stdout if enabled and no file specified
        if self.file.is_none() {
            println!("{}", trimmed);
        }

        // Write to file if configured
        if let Some(ref file_mutex) = self.file {
            if let Ok(mut file) = file_mutex.lock() {
                let _ = file.write_all(line.as_bytes());
                let _ = file.flush();
            }
        }

        // Store in the in-memory ring buffer
        if let Ok(mut buf) = self.memory_buffer.lock() {
            if buf.len() >= MEMORY_BUFFER_CAPACITY {
                buf.remove(0);
            }
            buf.push(trimmed.clone());
        }

        // Broadcast log entry to WebSocket subscribers.
        // Ignore send errors (no subscribers or channel full).
        let _ = self.log_tx.send(trimmed);
    }
}
