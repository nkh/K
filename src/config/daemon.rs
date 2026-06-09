use serde::{Deserialize, Serialize};

/// Daemon (background process) settings.
/// When enabled, vrc forks into the background after binding.
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
            stdout_file: "/tmp/vrc.out".to_string(),
            stderr_file: "/tmp/vrc.err".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_daemon_config() {
        let cfg = DaemonConfig::default();
        assert!(!cfg.enabled);
        assert_eq!(cfg.stdout_file, "/tmp/vrc.out");
        assert_eq!(cfg.stderr_file, "/tmp/vrc.err");
    }

    #[test]
    fn test_daemon_config_custom() {
        let cfg = DaemonConfig {
            enabled: true,
            stdout_file: "/var/log/vrc.out".to_string(),
            stderr_file: "/var/log/vrc.err".to_string(),
        };
        assert!(cfg.enabled);
        assert_eq!(cfg.stdout_file, "/var/log/vrc.out");
        assert_eq!(cfg.stderr_file, "/var/log/vrc.err");
    }

    #[test]
    fn test_daemon_config_serialization_roundtrip() {
        let cfg = DaemonConfig::default();
        let json = serde_json::to_string(&cfg).unwrap();
        let deserialized: DaemonConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.enabled, cfg.enabled);
        assert_eq!(deserialized.stdout_file, cfg.stdout_file);
        assert_eq!(deserialized.stderr_file, cfg.stderr_file);
    }

    #[test]
    fn test_daemon_config_serialization_with_enabled() {
        let cfg = DaemonConfig {
            enabled: true,
            stdout_file: "/tmp/daemon.out".to_string(),
            stderr_file: "/tmp/daemon.err".to_string(),
        };
        let json = serde_json::to_string(&cfg).unwrap();
        assert!(json.contains("\"enabled\":true"));
        assert!(json.contains("/tmp/daemon.out"));
    }

    #[test]
    fn test_daemon_config_debug_clone() {
        let cfg = DaemonConfig::default();
        let cloned = cfg.clone();
        assert_eq!(cfg.enabled, cloned.enabled);
        assert_eq!(cfg.stdout_file, cloned.stdout_file);
        let debug_str = format!("{:?}", cfg);
        assert!(debug_str.contains("DaemonConfig"));
    }
}
