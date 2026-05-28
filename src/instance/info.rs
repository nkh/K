use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceInfo {
    pub pid: u32,
    pub port: u16,
    pub bind: String,
    pub start_time: DateTime<Utc>,
    pub daemon: bool,
    pub display: bool,
    pub command: Option<String>,
}
