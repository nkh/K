use serde::{Deserialize, Serialize};

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
