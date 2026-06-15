use std::collections::HashMap;

use super::schema::{Config, PartialConfig};

/// Merge local config over global config.
/// Local fields fully override global (collections merge when empty local).
pub fn merge_configs(global: Config, local: Config) -> Config {
    Config {
        binary_name: local.binary_name,
        color_terminal_log: local.color_terminal_log,
        show_events: local.show_events,
        event_regexp: local.event_regexp,
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

fn merge_env(
    global: super::schema::EnvironmentConfig,
    local: super::schema::EnvironmentConfig,
) -> super::schema::EnvironmentConfig {
    let mut variables = global.variables;
    variables.extend(local.variables);
    super::schema::EnvironmentConfig { variables }
}

fn merge_profiles(
    global: super::schema::ProfilesConfig,
    local: super::schema::ProfilesConfig,
) -> super::schema::ProfilesConfig {
    let mut entries = global.entries;
    entries.extend(local.entries);
    super::schema::ProfilesConfig { entries }
}

/// Apply a named profile to a base configuration.
/// Only fields present (`Some`) in the partial config override the base.
pub fn apply_profile(base: Config, profile: &PartialConfig) -> Config {
    Config {
        binary_name: base.binary_name,
        color_terminal_log: base.color_terminal_log,
        show_events: base.show_events,
        event_regexp: base.event_regexp,
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

/// Merge per-command env vars over config-level defaults.
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
    fn test_merge_configs_local_overrides_global() {
        let global = Config::default();
        let mut local = Config::default();
        local.binary_name = "vrc".to_string();
        let merged = merge_configs(global, local);
        assert_eq!(merged.binary_name, "vrc");
    }

    #[test]
    fn test_apply_profile_binary_name_never_overridden() {
        let mut base = Config::default();
        base.binary_name = "vrc".to_string();
        let result = apply_profile(base, &PartialConfig::default());
        assert_eq!(result.binary_name, "vrc");
    }

    #[test]
    fn test_apply_profile_overrides_vtty() {
        let base = Config::default();
        let mut profile = PartialConfig::default();
        profile.vtty = Some(crate::config::schema::VttyConfig {
            rows: 50,
            cols: 200,
            ..Default::default()
        });
        let result = apply_profile(base, &profile);
        assert_eq!(result.vtty.rows, 50);
        assert_eq!(result.vtty.cols, 200);
    }
}
