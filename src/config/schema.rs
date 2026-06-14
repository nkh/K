use serde::{Deserialize, Serialize};

// Re-export remaining domain modules so that `config::schema::*` still works.
pub use super::display::{DisplayConfig, InteractiveConfig, KeybindingsConfig};
pub use super::hooks::{CommandLogConfig, DefaultExitConfig, ExitConfig, HooksConfig};
pub use super::templates::{TemplateConfig, TemplatesConfig};
pub use super::environments::{EnvironmentCommand, EnvironmentPanel, WorkspaceEnvironment, EnvironmentsConfig};

#[cfg(feature = "vrw")]
pub use super::security::{
    CertificateEntryConfig, CertificatesConfig, CorsConfig, SecurityConfig, TlsConfig,
};
#[cfg(feature = "vrw")]
pub use super::server::ServerConfig;
#[cfg(feature = "vrw")]
pub use super::web::{PanelColorEntry, WebConfig};

// ── daemon ──

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

// ── vtty ──

/// Virtual terminal configuration.
/// Controls the dimensions, TERM value, and capabilities of the pseudo-terminal
/// allocated for each spawned command.
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
    /// Default font size for PNG screenshots (vrw only).
    #[cfg(feature = "vrw")]
    pub screenshot_font_size: f32,
    /// Default font file path for PNG screenshots (vrw only).
    #[cfg(feature = "vrw")]
    pub screenshot_font_name: Option<String>,
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
            #[cfg(feature = "vrw")]
            screenshot_font_size: 14.0,
            #[cfg(feature = "vrw")]
            screenshot_font_name: None,
        }
    }
}

// ── handles ──

/// A pre-configured output handle.
/// Handles can be attached to spawned commands to direct their output
/// to a file, VTTY, or null sink by name.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HandleConfig {
    /// Name of the handle (used as the identifier in the API).
    pub name: String,
    /// Sink type: "file", "vtty", or "null".
    pub sink: String,
    /// Path for file sinks. Supports {id} and {name} placeholders.
    pub path: Option<String>,
}

// ── environment ──

/// Environment variable configuration for spawned commands.
///
/// Variables defined here are applied to every spawned command unless:
/// - The command is spawned with --no-env (CLI), which skips config env vars
/// - The variable is overridden by a per-command env var (API or CLI)
///
/// Per-command environment variables (from API or CLI --env flags) are always
/// merged on top of config environment variables, allowing overrides.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct EnvironmentConfig {
    /// Key-value pairs of environment variables to set in child processes.
    /// Example: { "RUST_LOG": "debug", "DATABASE_URL": "postgres://..." }
    #[serde(default)]
    pub variables: std::collections::HashMap<String, String>,
}

// ── profiles ──

/// Named configuration profiles.
///
/// Each profile is a partial configuration that can be selected by name.
/// When selected, only the fields present in the profile override the base config.
/// This allows defining reusable configurations for different environments
/// (e.g., "production", "development", "testing").
///
/// Example:
/// ```yaml
/// profiles:
///   production:
///     server:
///       bind: "0.0.0.0"
///     security:
///       require_auth: true
///     environment:
///       variables:
///         RUST_LOG: "warn"
///   development:
///     vtty:
///       rows: 40
///       cols: 120
///     environment:
///       variables:
///         RUST_LOG: "debug"
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ProfilesConfig {
    /// Named configuration presets. The key is the profile name.
    #[serde(default)]
    pub entries: std::collections::HashMap<String, PartialConfig>,
}

/// Top-level configuration.
///
/// All fields have sensible defaults, so a config file is entirely optional.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Config {
    /// Binary name ("vrw" or "vrc") — set at runtime from CLI, not from config file.
    #[serde(default, skip)]
    pub binary_name: String,
    /// Whether to use ANSI color codes in terminal log output.
    /// Set at runtime from the `--color-terminal-log` / `-F` CLI flag.
    #[serde(default, skip)]
    pub color_terminal_log: bool,

    /// HTTP server bind address and port (vrw only).
    #[cfg(feature = "vrw")]
    #[serde(default)]
    pub server: ServerConfig,
    /// Authentication settings (vrw only).
    #[cfg(feature = "vrw")]
    #[serde(default)]
    pub security: SecurityConfig,
    /// TLS/HTTPS certificate settings (vrw only).
    #[cfg(feature = "vrw")]
    #[serde(default)]
    pub tls: TlsConfig,
    /// Per-command client certificate pool (vrw only).
    #[cfg(feature = "vrw")]
    #[serde(default)]
    pub certificates: CertificatesConfig,
    /// Virtual terminal (VTTY) dimensions and behavior.
    #[serde(default)]
    pub vtty: VttyConfig,
    /// Local terminal display of VTTY output.
    #[serde(default)]
    pub display: DisplayConfig,
    /// Logging of API command events.
    #[serde(default)]
    pub command_log: CommandLogConfig,
    /// Daemon (background) mode settings.
    #[serde(default)]
    pub daemon: DaemonConfig,
    /// Pre-configured output handles for spawned commands.
    #[serde(default)]
    pub handles: Vec<HandleConfig>,
    /// Interactive terminal display settings (tab bar, keyboard).
    #[serde(default)]
    pub interactive: InteractiveConfig,
    /// Default exit configuration applied to all commands unless overridden per-command.
    #[serde(default)]
    pub default_exit: DefaultExitConfig,
    /// Global event hooks — shell commands triggered on lifecycle events.
    #[serde(default)]
    pub hooks: HooksConfig,
    /// Environment variables applied to all spawned commands by default.
    #[serde(default)]
    pub environment: EnvironmentConfig,
    /// Web admin panel and VTTY streaming configuration (vrw only).
    #[cfg(feature = "vrw")]
    #[serde(default)]
    pub web: WebConfig,
    /// Pre-defined command templates.
    #[serde(default)]
    pub templates: TemplatesConfig,
    /// Named workspace environments (panels, servers, commands).
    #[serde(default)]
    pub environments: EnvironmentsConfig,
    /// Named configuration presets.
    #[serde(default)]
    pub profiles: ProfilesConfig,
}

/// A partial configuration used in profiles.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct PartialConfig {
    #[cfg(feature = "vrw")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server: Option<ServerConfig>,
    #[cfg(feature = "vrw")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub security: Option<SecurityConfig>,
    #[cfg(feature = "vrw")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls: Option<TlsConfig>,
    #[cfg(feature = "vrw")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub certificates: Option<CertificatesConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vtty: Option<VttyConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display: Option<DisplayConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command_log: Option<CommandLogConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handles: Option<Vec<HandleConfig>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interactive: Option<InteractiveConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_exit: Option<DefaultExitConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment: Option<EnvironmentConfig>,
    #[cfg(feature = "vrw")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub web: Option<WebConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hooks: Option<HooksConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub templates: Option<TemplatesConfig>,
}
