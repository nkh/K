use serde::{Deserialize, Serialize};

// Re-export all domain modules so that `config::schema::*` still works
// for every existing import site.
pub use super::daemon::DaemonConfig;
pub use super::display::{DisplayConfig, InteractiveConfig, KeybindingsConfig};
pub use super::environment::EnvironmentConfig;
pub use super::handles::HandleConfig;
pub use super::hooks::{CommandLogConfig, DefaultExitConfig, ExitConfig, HooksConfig};
pub use super::profiles::ProfilesConfig;
pub use super::templates::{TemplateConfig, TemplatesConfig};
pub use super::vtty::VttyConfig;

#[cfg(feature = "vrunner")]
pub use super::security::{
    CertificateEntryConfig, CertificatesConfig, CorsConfig, SecurityConfig, TlsConfig,
};
#[cfg(feature = "vrunner")]
pub use super::server::ServerConfig;
#[cfg(feature = "vrunner")]
pub use super::web::{RateLimitConfig, WebConfig};

/// Top-level configuration.
///
/// All fields have sensible defaults, so a config file is entirely optional.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Config {
    /// HTTP server bind address and port (vrunner only).
    #[cfg(feature = "vrunner")]
    #[serde(default)]
    pub server: ServerConfig,
    /// Authentication settings (vrunner only).
    #[cfg(feature = "vrunner")]
    #[serde(default)]
    pub security: SecurityConfig,
    /// TLS/HTTPS certificate settings (vrunner only).
    #[cfg(feature = "vrunner")]
    #[serde(default)]
    pub tls: TlsConfig,
    /// Per-command client certificate pool (vrunner only).
    #[cfg(feature = "vrunner")]
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
    /// Web admin panel and VTTY streaming configuration (vrunner only).
    #[cfg(feature = "vrunner")]
    #[serde(default)]
    pub web: WebConfig,
    /// Pre-defined command templates.
    #[serde(default)]
    pub templates: TemplatesConfig,
    /// Named configuration presets.
    #[serde(default)]
    pub profiles: ProfilesConfig,
}

/// A partial configuration used in profiles.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct PartialConfig {
    #[cfg(feature = "vrunner")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server: Option<ServerConfig>,
    #[cfg(feature = "vrunner")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub security: Option<SecurityConfig>,
    #[cfg(feature = "vrunner")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls: Option<TlsConfig>,
    #[cfg(feature = "vrunner")]
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
    #[cfg(feature = "vrunner")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub web: Option<WebConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hooks: Option<HooksConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub templates: Option<TemplatesConfig>,
}
