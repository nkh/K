#![cfg(feature = "vrw")]
#![allow(dead_code, unused_imports)]
use serde::{Deserialize, Serialize};

/// Cross-Origin Resource Sharing (CORS) configuration.
///
/// Controls which origins are allowed to make cross-origin requests to the
/// vrw API and admin interface.
///
/// # Example (YAML)
///
/// ```yaml
/// security:
///   cors:
///     policy: "https://myapp.example.com,https://admin.example.com"
/// ```
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CorsConfig {
    /// CORS policy. Determines which origins are allowed for cross-origin requests.
    ///
    /// - `"any"` — allow all origins (default, backward compatible).
    /// - `"none"` — block all cross-origin requests by not setting any
    ///   `Access-Control-Allow-Origin` header.
    /// - A comma-separated list of allowed origins for fine-grained control.
    ///   Example: `"https://myapp.example.com,https://admin.example.com"`
    #[serde(default = "default_cors_policy")]
    pub policy: String,
}

fn default_cors_policy() -> String {
    "any".to_string()
}

impl Default for CorsConfig {
    fn default() -> Self {
        Self {
            policy: default_cors_policy(),
        }
    }
}

/// Authentication and authorization settings.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SecurityConfig {
    /// When false (default), no authentication is required.
    /// When true, a bearer token must be provided in the Authorization header.
    /// This should be enabled when server.bind is set to 0.0.0.0.
    pub require_auth: bool,
    /// Path to a file containing the bearer token. If the file does not exist
    /// when auth is required, a random 256-bit token is generated and saved.
    /// Default: ~/.config/vrw/token
    pub token_file: String,
    /// CORS (Cross-Origin Resource Sharing) configuration.
    /// Controls which origins may make cross-origin requests.
    /// Default: allow all origins.
    #[serde(default)]
    pub cors: CorsConfig,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            require_auth: false,
            token_file: dirs::config_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
                .join("vrw")
                .join("token")
                .to_string_lossy()
                .to_string(),
            cors: CorsConfig::default(),
        }
    }
}

/// TLS/HTTPS settings.
/// When enabled without explicit cert/key paths, vrw auto-generates
/// self-signed certificates stored in ~/.config/vrw/.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct TlsConfig {
    /// Enable TLS (HTTPS). Default: false.
    /// When enabled, vrw generates self-signed certificates on first run
    /// (or uses existing ones). The certificate and key are stored in
    /// ~/.config/vrw/.
    #[serde(default)]
    pub enabled: bool,
    /// Path to the PEM-encoded certificate file.
    /// If not set, defaults to ~/.config/vrw/cert.pem.
    pub cert_file: Option<String>,
    /// Path to the PEM-encoded private key file.
    /// If not set, defaults to ~/.config/vrw/key.pem.
    pub key_file: Option<String>,
}

/// Configuration for the certificate pool.
///
/// Each entry defines a named certificate that can be bound to running commands.
/// When a command is bound to a certificate, only clients presenting that
/// certificate (or its derived bearer token) can interact with the command.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct CertificatesConfig {
    /// Directory where auto-generated certificates are stored.
    /// Default: ~/.config/vrw/certs/
    #[serde(default)]
    pub directory: Option<String>,
    /// Named certificate definitions.
    /// Each entry has a name, cert_file, and key_file.
    /// Missing files are auto-generated on first use.
    #[serde(default)]
    pub entries: Vec<CertificateEntryConfig>,
}

/// A single named certificate in the pool.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CertificateEntryConfig {
    /// Logical name for this certificate (e.g., "webapp-frontend").
    pub name: String,
    /// Path to the PEM-encoded certificate file.
    /// Can be absolute or relative to certificates.directory.
    #[serde(default)]
    pub cert_file: String,
    /// Path to the PEM-encoded private key file.
    /// Can be absolute or relative to certificates.directory.
    #[serde(default)]
    pub key_file: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_security_config_default() {
        let cfg = SecurityConfig::default();
        assert!(!cfg.require_auth);
        assert!(cfg.token_file.contains("vrw"));
        assert!(cfg.token_file.contains("token"));
    }

    #[test]
    fn test_cors_config_default() {
        let cfg = CorsConfig::default();
        assert_eq!(cfg.policy, "any");
    }

    #[test]
    fn test_cors_config_custom() {
        let cfg = CorsConfig {
            policy: "none".to_string(),
        };
        assert_eq!(cfg.policy, "none");
    }

    #[test]
    fn test_tls_config_default() {
        let cfg = TlsConfig::default();
        assert!(!cfg.enabled);
        assert!(cfg.cert_file.is_none());
        assert!(cfg.key_file.is_none());
    }

    #[test]
    fn test_tls_config_custom() {
        let cfg = TlsConfig {
            enabled: true,
            cert_file: Some("/path/to/cert.pem".to_string()),
            key_file: Some("/path/to/key.pem".to_string()),
        };
        assert!(cfg.enabled);
        assert_eq!(cfg.cert_file, Some("/path/to/cert.pem".to_string()));
        assert_eq!(cfg.key_file, Some("/path/to/key.pem".to_string()));
    }

    #[test]
    fn test_certificates_config_default() {
        let cfg = CertificatesConfig::default();
        assert!(cfg.directory.is_none());
        assert!(cfg.entries.is_empty());
    }

    #[test]
    fn test_certificate_entry_config() {
        let entry = CertificateEntryConfig {
            name: "webapp".to_string(),
            cert_file: "/certs/webapp.pem".to_string(),
            key_file: "/certs/webapp-key.pem".to_string(),
        };
        assert_eq!(entry.name, "webapp");
        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: CertificateEntryConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, entry.name);
        assert_eq!(deserialized.cert_file, entry.cert_file);
        assert_eq!(deserialized.key_file, entry.key_file);
    }

    #[test]
    fn test_security_config_serialization_roundtrip() {
        let cfg = SecurityConfig {
            require_auth: true,
            token_file: "/tmp/token".to_string(),
            cors: CorsConfig { policy: "none".to_string() },
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let deserialized: SecurityConfig = serde_json::from_str(&json).unwrap();
        assert!(deserialized.require_auth);
        assert_eq!(deserialized.token_file, "/tmp/token");
        assert_eq!(deserialized.cors.policy, "none");
    }

    #[test]
    fn test_tls_config_serialization_roundtrip() {
        let cfg = TlsConfig {
            enabled: true,
            cert_file: Some("/cert.pem".to_string()),
            key_file: Some("/key.pem".to_string()),
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let deserialized: TlsConfig = serde_json::from_str(&json).unwrap();
        assert!(deserialized.enabled);
        assert_eq!(deserialized.cert_file, Some("/cert.pem".to_string()));
    }
}
