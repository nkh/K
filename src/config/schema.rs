use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Config {
    pub server: ServerConfig,
    pub security: SecurityConfig,
    pub tls: TlsConfig,
    pub certificates: CertificatesConfig,
    pub vtty: VttyConfig,
    pub display: DisplayConfig,
    pub command_log: CommandLogConfig,
    pub daemon: DaemonConfig,
    pub handles: Vec<HandleConfig>,
}

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
            port: 8080,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SecurityConfig {
    /// When false (default), no authentication is required.
    /// When true, a bearer token must be provided in the Authorization header.
    /// This should be enabled when server.bind is set to 0.0.0.0.
    pub require_auth: bool,
    /// Path to a file containing the bearer token. If the file does not exist
    /// when auth is required, a random 256-bit token is generated and saved.
    /// Default: ~/.config/vrunner/token
    pub token_file: String,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            require_auth: false,
            token_file: dirs::config_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
                .join("vrunner")
                .join("token")
                .to_string_lossy()
                .to_string(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct TlsConfig {
    /// Enable TLS (HTTPS). Default: false.
    /// When enabled, vrunner generates self-signed certificates on first run
    /// (or uses existing ones). The certificate and key are stored in
    /// ~/.config/vrunner/.
    #[serde(default)]
    pub enabled: bool,
    /// Path to the PEM-encoded certificate file.
    /// If not set, defaults to ~/.config/vrunner/cert.pem.
    pub cert_file: Option<String>,
    /// Path to the PEM-encoded private key file.
    /// If not set, defaults to ~/.config/vrunner/key.pem.
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
    /// Default: ~/.config/vrunner/certs/
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
    pub cert_file: String,
    /// Path to the PEM-encoded private key file.
    /// Can be absolute or relative to certificates.directory.
    pub key_file: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VttyConfig {
    /// Number of rows in the virtual terminal.
    pub rows: u16,
    /// Number of columns in the virtual terminal.
    pub cols: u16,
    /// The TERM value reported to child processes.
    pub term: String,
    /// Maximum number of scrollback lines retained.
    pub scrollback: usize,
    /// Enable 24-bit truecolor support.
    pub truecolor: bool,
    /// Enable mouse event forwarding.
    pub mouse: bool,
}

impl Default for VttyConfig {
    fn default() -> Self {
        Self {
            rows: 24,
            cols: 80,
            term: "xterm-256color".to_string(),
            scrollback: 5000,
            truecolor: true,
            mouse: false,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DisplayConfig {
    /// Show VTTY output on the local terminal.
    pub enabled: bool,
    /// Refresh interval in milliseconds when display is enabled.
    pub refresh_ms: u64,
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            refresh_ms: 100,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct CommandLogConfig {
    /// Enable logging of API commands.
    pub enabled: bool,
    /// Path to the command log file. If set, logs are written to this file
    /// in addition to the terminal.
    pub file: Option<String>,
}

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
            stdout_file: "/tmp/vrunner.out".to_string(),
            stderr_file: "/tmp/vrunner.err".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HandleConfig {
    /// Name of the handle (used as the identifier in the API).
    pub name: String,
    /// Sink type: "file", "vtty", or "null".
    pub sink: String,
    /// Path for file sinks. Supports {id} and {name} placeholders.
    pub path: Option<String>,
}
