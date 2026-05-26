use serde::{Deserialize, Serialize};

use super::schema::PartialConfig;

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
