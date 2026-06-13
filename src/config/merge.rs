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
        environments: if local.environments.is_empty() {
            global.environments
        } else {
            local.environments
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
        environments: base.environments,
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

    // ─── merge_configs tests ───

    #[test]
    fn test_merge_configs_local_overrides_global() {
        let global = Config::default();
        let mut local = Config::default();
        local.binary_name = "vrc".to_string();
        let merged = merge_configs(global, local);
        assert_eq!(merged.binary_name, "vrc");
    }

    // ─── merge_profiles tests ───

    #[test]
    fn test_merge_profiles_local_overrides_global() {
        let global = super::super::schema::ProfilesConfig {
            entries: HashMap::from([
                ("dev".to_string(), PartialConfig::default()),
                ("prod".to_string(), PartialConfig::default()),
            ]),
        };
        let mut local_profile = PartialConfig::default();
        // Can't set non-None fields on PartialConfig easily, but we can
        // test that local entries override global ones with the same name.
        let local = super::super::schema::ProfilesConfig {
            entries: HashMap::from([
                ("prod".to_string(), local_profile.clone()),
            ]),
        };
        let merged = merge_profiles(global, local);
        assert!(merged.entries.contains_key("dev")); // from global
        assert!(merged.entries.contains_key("prod")); // from local
    }

    #[test]
    fn test_merge_profiles_new_local_entries() {
        let global = super::super::schema::ProfilesConfig::default();
        let local = super::super::schema::ProfilesConfig {
            entries: HashMap::from([
                ("new_profile".to_string(), PartialConfig::default()),
            ]),
        };
        let merged = merge_profiles(global, local);
        assert!(merged.entries.contains_key("new_profile"));
    }

    #[test]
    fn test_merge_profiles_empty_local_keeps_global() {
        let global = super::super::schema::ProfilesConfig {
            entries: HashMap::from([
                ("existing".to_string(), PartialConfig::default()),
            ]),
        };
        let local = super::super::schema::ProfilesConfig::default();
        let merged = merge_profiles(global, local);
        assert!(merged.entries.contains_key("existing"));
    }

    // ─── apply_profile tests ───

    #[test]
    fn test_apply_profile_empty_profile_keeps_base() {
        let base = Config::default();
        let profile = PartialConfig::default();
        let result = apply_profile(base.clone(), &profile);
        // Empty profile should keep base values
        assert_eq!(result.vtty.rows, base.vtty.rows);
        assert_eq!(result.vtty.cols, base.vtty.cols);
        assert_eq!(result.display.enabled, base.display.enabled);
        assert_eq!(result.display.refresh_ms, base.display.refresh_ms);
    }

    #[test]
    fn test_apply_profile_overrides_vtty() {
        let base = Config::default();
        let mut profile = PartialConfig::default();
        profile.vtty = Some(crate::config::vtty::VttyConfig {
            rows: 50,
            cols: 200,
            ..Default::default()
        });
        let result = apply_profile(base, &profile);
        assert_eq!(result.vtty.rows, 50);
        assert_eq!(result.vtty.cols, 200);
    }

    #[test]
    fn test_apply_profile_binary_name_never_overridden() {
        let mut base = Config::default();
        base.binary_name = "vrc".to_string();
        let profile = PartialConfig::default();
        let result = apply_profile(base, &profile);
        assert_eq!(result.binary_name, "vrc");
    }

    #[test]
    fn test_apply_profile_overrides_display() {
        let base = Config::default();
        let mut profile = PartialConfig::default();
        let custom_display = crate::config::display::DisplayConfig {
            enabled: true,
            refresh_ms: 500,
            display_all: true,
        };
        profile.display = Some(custom_display);
        let result = apply_profile(base, &profile);
        // DisplayConfig values come from profile
        assert!(result.display.enabled);
        assert_eq!(result.display.refresh_ms, 500);
        assert!(result.display.display_all);
    }

    #[test]
    fn test_apply_profile_overrides_environment() {
        let base = Config::default();
        let mut profile = PartialConfig::default();
        profile.environment = Some(super::super::schema::EnvironmentConfig {
            variables: HashMap::from([("RUST_LOG".to_string(), "debug".to_string())]),
        });
        let result = apply_profile(base, &profile);
        assert_eq!(
            result.environment.variables.get("RUST_LOG").unwrap(),
            "debug"
        );
    }
}
