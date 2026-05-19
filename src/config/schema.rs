use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub server: ServerConfig,
    pub vtty: VttyConfig,
    pub display: DisplayConfig,
    pub command_log: CommandLogConfig,
    pub daemon: DaemonConfig,
    pub handles: Vec<HandleConfig>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            vtty: VttyConfig::default(),
            display: DisplayConfig::default(),
            command_log: CommandLogConfig::default(),
            daemon: DaemonConfig::default(),
            handles: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    pub bind: String,
    pub port: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind: "127.0.0.1".to_string(),
            port: 8080,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VttyConfig {
    pub rows: u16,
    pub cols: u16,
    pub term: String,
    pub scrollback: usize,
    pub truecolor: bool,
    pub mouse: bool,
}

impl Default for VttyConfig {
    fn default() -> Self {
        Self {
            rows: 24,
            cols: 80,
            term: "xterm-256color".to_string(),
            scrollback: 5000,
            truecolor: true,
            mouse: false,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DisplayConfig {
    pub enabled: bool,
    pub refresh_ms: u64,
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            refresh_ms: 100,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CommandLogConfig {
    pub enabled: bool,
    pub file: Option<String>,
}

impl Default for CommandLogConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            file: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DaemonConfig {
    pub enabled: bool,
    pub stdout_file: String,
    pub stderr_file: String,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            stdout_file: "/tmp/vrunner.out".to_string(),
            stderr_file: "/tmp/vrunner.err".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HandleConfig {
    pub name: String,
    pub sink: String,
    pub path: Option<String>,
}
