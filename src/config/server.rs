#![cfg(feature = "vrunner")]
#![allow(dead_code, unused_imports)]
use serde::{Deserialize, Serialize};

/// HTTP server bind address and port.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    /// Bind address. Default "127.0.0.1" (localhost only).
    /// Set to "0.0.0.0" to allow remote connections.
    pub bind: String,
    /// TCP port to listen on.
    pub port: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind: "127.0.0.1".to_string(),
            port: 9090,
        }
    }
}
