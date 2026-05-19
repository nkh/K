use super::schema::Config;

pub fn merge_configs(global: Config, local: Config) -> Config {
    Config {
        server: local.server,
        vtty: local.vtty,
        display: local.display,
        command_log: local.command_log,
        daemon: local.daemon,
        handles: if local.handles.is_empty() { global.handles } else { local.handles },
    }
}
