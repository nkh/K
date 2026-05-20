use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Mutex;
use chrono::Utc;
use tokio::sync::broadcast;

pub struct CommandLogger {
    enabled: bool,
    file: Option<Mutex<std::fs::File>>,
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
        Ok(Self { enabled, file, log_tx })
    }

    /// Subscribe to log entries. Returns a broadcast receiver.
    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.log_tx.subscribe()
    }

    /// Get a clone of the log broadcast sender.
    pub fn log_sender(&self) -> broadcast::Sender<String> {
        self.log_tx.clone()
    }

    pub fn log(&self, command: &str, details: &str) {
        if !self.enabled {
            return;
        }
        let timestamp = Utc::now().to_rfc3339();
        let line = format!("[{}] {}: {}
", timestamp, command, details);

        // Always print to stdout if enabled and no file specified
        if self.file.is_none() {
            println!("{}", line.trim_end());
        }

        // Write to file if configured
        if let Some(ref file_mutex) = self.file {
            if let Ok(mut file) = file_mutex.lock() {
                let _ = file.write_all(line.as_bytes());
                let _ = file.flush();
            }
        }

        // Broadcast log entry to WebSocket subscribers.
        // Ignore send errors (no subscribers or channel full).
        let _ = self.log_tx.send(line.trim_end().to_string());
    }
}
