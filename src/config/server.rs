#![cfg(feature = "vrw")]
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
    /// Human-readable name for this server instance.
    /// Displayed in `vrw list`, `vrw cat`, and the web UI panel titlebar.
    /// Falls back to "host:port" when not set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind: "127.0.0.1".to_string(),
            port: 9090,
            name: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_server_config() {
        let cfg = ServerConfig::default();
        assert_eq!(cfg.bind, "127.0.0.1");
        assert_eq!(cfg.port, 9090);
        assert!(cfg.name.is_none());
    }

    #[test]
    fn test_server_config_custom() {
        let cfg = ServerConfig {
            bind: "0.0.0.0".to_string(),
            port: 8080,
            name: Some("Production".to_string()),
        };
        assert_eq!(cfg.bind, "0.0.0.0");
        assert_eq!(cfg.port, 8080);
        assert_eq!(cfg.name, Some("Production".to_string()));
    }

    #[test]
    fn test_server_config_serialization_roundtrip() {
        let cfg = ServerConfig::default();
        let json = serde_json::to_string(&cfg).unwrap();
        let deserialized: ServerConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.bind, cfg.bind);
        assert_eq!(deserialized.port, cfg.port);
        assert_eq!(deserialized.name, cfg.name);
    }

    #[test]
    fn test_server_config_name_skipped_when_none() {
        let cfg = ServerConfig::default();
        let json = serde_json::to_string(&cfg).unwrap();
        assert!(!json.contains("name"));
    }

    #[test]
    fn test_server_config_name_included_when_some() {
        let cfg = ServerConfig {
            bind: "127.0.0.1".to_string(),
            port: 9090,
            name: Some("test".to_string()),
        };
        let json = serde_json::to_string(&cfg).unwrap();
        assert!(json.contains("name"));
        assert!(json.contains("test"));
    }
}
