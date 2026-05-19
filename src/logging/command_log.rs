use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Mutex;
use chrono::Utc;

pub struct CommandLogger {
    enabled: bool,
    file: Option<Mutex<std::fs::File>>,
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
        Ok(Self { enabled, file })
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
            return;
        }

        // Write to file if configured
        if let Some(ref file_mutex) = self.file {
            if let Ok(mut file) = file_mutex.lock() {
                let _ = file.write_all(line.as_bytes());
                let _ = file.flush();
            }
        }
    }
}
