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

/// Top-level configuration for vrl.
///
/// All fields have sensible defaults, so a config file is entirely optional.
/// When no config file is present, vrl runs with the default VTTY
/// dimensions and no display.
///
/// Config files are searched in this order (later files override earlier):
/// 1. ~/.config/vrl/config.yaml (or .toml)
/// 2. ./vrl.yaml (or .toml) in the current directory
/// 3. Path specified with --config CLI flag
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Config {
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
    /// Global event hooks -- shell commands triggered on lifecycle events.
    #[serde(default)]
    pub hooks: HooksConfig,
    /// Environment variables applied to all spawned commands by default.
    /// Can be overridden per-command via the API or CLI.
    /// Ignored when --no-env is passed on the command line.
    #[serde(default)]
    pub environment: EnvironmentConfig,
    /// Pre-defined command templates.
    #[serde(default)]
    pub templates: TemplatesConfig,
    /// Named configuration presets.
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
    pub hooks: Option<HooksConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub templates: Option<TemplatesConfig>,
}
