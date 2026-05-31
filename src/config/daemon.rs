use serde::{Deserialize, Serialize};

/// Daemon (background process) settings.
/// When enabled, vrl forks into the background after binding.
/// Only available on Unix systems.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DaemonConfig {
    /// Run as a background daemon (Unix only).
    pub enabled: bool,
    /// File to redirect stdout to when daemonized.
    pub stdout_file: String,
    /// File to redirect stderr to when daemonized.
    pub stderr_file: String,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            stdout_file: "/tmp/vrl.out".to_string(),
            stderr_file: "/tmp/vrl.err".to_string(),
        }
    }
}
