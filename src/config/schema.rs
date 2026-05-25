use serde::{Deserialize, Serialize};

// Re-export all domain modules so that `config::schema::*` still works
// for every existing import site.
pub use super::daemon::DaemonConfig;
pub use super::display::{DisplayConfig, InteractiveConfig, KeybindingsConfig};
pub use super::environment::EnvironmentConfig;
pub use super::handles::HandleConfig;
pub use super::hooks::{CommandLogConfig, DefaultExitConfig, ExitConfig, HooksConfig};
pub use super::security::{CertificateEntryConfig, CertificatesConfig, SecurityConfig, TlsConfig};
pub use super::server::ServerConfig;
pub use super::vtty::VttyConfig;
pub use super::web::{RateLimitConfig, WebConfig};
pub use super::profiles::ProfilesConfig;

/// Top-level configuration for vrunner.
///
/// All fields have sensible defaults, so a config file is entirely optional.
/// When no config file is present, vrunner runs with localhost-only HTTP on
/// port 9090, no authentication, and no TLS.
///
/// Config files are searched in this order (later files override earlier):
/// 1. ~/.config/vrunner/config.yaml (or .toml)
/// 2. ./vrunner.yaml (or .toml) in the current directory
/// 3. Path specified with --config CLI flag
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Config {
    /// HTTP server bind address and port.
    #[serde(default)]
    pub server: ServerConfig,
    /// Authentication settings (bearer token).
    #[serde(default)]
    pub security: SecurityConfig,
    /// TLS/HTTPS certificate settings.
    #[serde(default)]
    pub tls: TlsConfig,
    /// Per-command client certificate pool.
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
    /// Can be overridden per-command via the API or CLI.
    /// Ignored when --no-env is passed on the command line.
    #[serde(default)]
    pub environment: EnvironmentConfig,
    /// Web admin panel and VTTY streaming configuration.
    /// Controls how the web UI discovers buffer changes (push vs poll),
    /// the dirty-check interval on the server, and the default polling
    /// interval for clients that use poll mode.
    #[serde(default)]
    pub web: WebConfig,
    /// Named configuration presets.
    /// Each named config is a partial Config that can be referenced by name
    /// via --profile NAME (CLI) or "profile": "NAME" (API).
    /// When a profile is selected, only the fields present in the named config
    /// override the base config. CLI flags always take final precedence.
    #[serde(default)]
    pub profiles: ProfilesConfig,
}

/// A partial configuration used in profiles.
///
/// All fields are optional. When a profile is applied, only the fields
/// that are `Some(..)` override the corresponding fields in the base Config.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct PartialConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server: Option<ServerConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub security: Option<SecurityConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls: Option<TlsConfig>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub web: Option<WebConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hooks: Option<HooksConfig>,
}
