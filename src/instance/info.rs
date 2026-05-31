use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceInfo {
    pub pid: u32,
    /// HTTP port (vrunner only).
    #[cfg(feature = "vrunner")]
    pub port: u16,
    /// Bind address (vrunner only).
    #[cfg(feature = "vrunner")]
    pub bind: String,
    pub start_time: DateTime<Utc>,
    pub daemon: bool,
    pub display: bool,
    /// Startup command name (vrunner only).
    #[cfg(feature = "vrunner")]
    pub command: Option<String>,
}
