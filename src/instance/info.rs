use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceInfo {
    pub pid: u32,
    /// HTTP port (vrw only).
    #[cfg(feature = "vrw")]
    pub port: u16,
    /// Bind address (vrw only).
    #[cfg(feature = "vrw")]
    pub bind: String,
    /// Human-readable server name (vrw only).
    #[cfg(feature = "vrw")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub start_time: DateTime<Utc>,
    pub daemon: bool,
    pub display: bool,
    /// Startup command name (vrw only).
    #[cfg(feature = "vrw")]
    pub command: Option<String>,
}
