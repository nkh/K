use super::schema::Config;

pub fn merge_configs(global: Config, local: Config) -> Config {
    Config {
        server: local.server,
        security: local.security,
        tls: local.tls,
        certificates: if local.certificates.entries.is_empty() { global.certificates } else { local.certificates },
        vtty: local.vtty,
        display: local.display,
        command_log: local.command_log,
        daemon: local.daemon,
        handles: if local.handles.is_empty() { global.handles } else { local.handles },
        interactive: local.interactive,
        default_exit: local.default_exit,
    }
}
