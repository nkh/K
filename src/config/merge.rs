use std::collections::HashMap;

use super::schema::{Config, PartialConfig};

/// Merge local config over global config.
/// Local fields fully override global (except collections which merge when empty).
pub fn merge_configs(global: Config, local: Config) -> Config {
    Config {
        binary_name: local.binary_name,
        color_terminal_log: local.color_terminal_log,
        vtty: local.vtty,
        display: local.display,
        command_log: local.command_log,
        daemon: local.daemon,
        handles: if local.handles.is_empty() {
            global.handles
        } else {
            local.handles
        },
        interactive: local.interactive,
        default_exit: local.default_exit,
        environment: merge_env(global.environment, local.environment),
        hooks: local.hooks,
        templates: if local.templates.is_empty() {
            global.templates
        } else {
            local.templates
        },
        profiles: merge_profiles(global.profiles, local.profiles),
        #[cfg(feature = "vrw")]
        server: local.server,
        #[cfg(feature = "vrw")]
        security: local.security,
        #[cfg(feature = "vrw")]
        tls: local.tls,
        #[cfg(feature = "vrw")]
        certificates: local.certificates,
        #[cfg(feature = "vrw")]
        web: local.web,
    }
}

/// Merge environment variables: local overrides global, global provides defaults.
fn merge_env(
    global: super::schema::EnvironmentConfig,
    local: super::schema::EnvironmentConfig,
) -> super::schema::EnvironmentConfig {
    let mut variables = global.variables;
    variables.extend(local.variables);
    super::schema::EnvironmentConfig { variables }
}

/// Merge profiles: local entries override global entries with the same name.
fn merge_profiles(
    global: super::schema::ProfilesConfig,
    local: super::schema::ProfilesConfig,
) -> super::schema::ProfilesConfig {
    let mut entries = global.entries;
    entries.extend(local.entries);
    super::schema::ProfilesConfig { entries }
}

/// Apply a named profile to a base configuration.
/// Only fields present (Some) in the partial config override the base.
pub fn apply_profile(base: Config, profile: &PartialConfig) -> Config {
    Config {
        binary_name: base.binary_name,
        color_terminal_log: base.color_terminal_log,
        vtty: profile.vtty.clone().unwrap_or(base.vtty),
        display: profile.display.clone().unwrap_or(base.display),
        command_log: profile.command_log.clone().unwrap_or(base.command_log),
        daemon: base.daemon,
        handles: profile.handles.clone().unwrap_or(base.handles),
        interactive: profile.interactive.clone().unwrap_or(base.interactive),
        default_exit: profile.default_exit.clone().unwrap_or(base.default_exit),
        environment: profile.environment.clone().unwrap_or(base.environment),
        hooks: profile.hooks.clone().unwrap_or(base.hooks),
        templates: profile.templates.clone().unwrap_or(base.templates),
        profiles: base.profiles,
        #[cfg(feature = "vrw")]
        server: profile.server.clone().unwrap_or(base.server),
        #[cfg(feature = "vrw")]
        security: profile.security.clone().unwrap_or(base.security),
        #[cfg(feature = "vrw")]
        tls: profile.tls.clone().unwrap_or(base.tls),
        #[cfg(feature = "vrw")]
        certificates: profile.certificates.clone().unwrap_or(base.certificates),
        #[cfg(feature = "vrw")]
        web: profile.web.clone().unwrap_or(base.web),
    }
}

/// Merge per-command environment variables on top of config-level defaults.
/// Per-command values always take precedence over config values.
pub fn merge_command_env(
    config_env: &super::schema::EnvironmentConfig,
    command_env: HashMap<String, String>,
) -> HashMap<String, String> {
    let mut merged = config_env.variables.clone();
    merged.extend(command_env);
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_env_local_overrides_global() {
        let global = super::super::schema::EnvironmentConfig {
            variables: HashMap::from([
                ("A".to_string(), "1".to_string()),
                ("B".to_string(), "2".to_string()),
            ]),
        };
        let local = super::super::schema::EnvironmentConfig {
            variables: HashMap::from([
                ("B".to_string(), "20".to_string()),
                ("C".to_string(), "3".to_string()),
            ]),
        };
        let merged = merge_env(global, local);
        assert_eq!(merged.variables.get("A").unwrap(), "1");
        assert_eq!(merged.variables.get("B").unwrap(), "20");
        assert_eq!(merged.variables.get("C").unwrap(), "3");
    }

    #[test]
    fn test_merge_command_env_overrides_config() {
        let config_env = super::super::schema::EnvironmentConfig {
            variables: HashMap::from([
                ("X".to_string(), "global".to_string()),
                ("Y".to_string(), "global".to_string()),
            ]),
        };
        let command_env = HashMap::from([
            ("Y".to_string(), "local".to_string()),
            ("Z".to_string(), "local".to_string()),
        ]);
        let merged = merge_command_env(&config_env, command_env);
        assert_eq!(merged.get("X").unwrap(), "global");
        assert_eq!(merged.get("Y").unwrap(), "local");
        assert_eq!(merged.get("Z").unwrap(), "local");
    }
}
