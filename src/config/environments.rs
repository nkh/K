use serde::{Deserialize, Serialize};

/// A single command to spawn within an environment panel.
///
/// Each panel in an environment can have zero or more commands.
/// Commands are spawned sequentially in the order listed.
/// The first command's VTTY is displayed in the panel.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EnvironmentCommand {
    /// The command executable to run.
    ///
    /// Example: `"npm"`, `"/usr/bin/htop"`, `"cargo"`
    pub cmd: String,

    /// Space-separated arguments passed to the command.
    ///
    /// Example: `"run dev"`, `"--sort-key PID"`
    #[serde(default)]
    pub args: Option<String>,

    /// Working directory for the spawned command.
    ///
    /// Optional — defaults to the server's working directory.
    #[serde(default)]
    pub workdir: Option<String>,

    /// Certificate name to bind (from the server's certificates).
    ///
    /// Optional — defaults to no certificate binding.
    #[serde(default)]
    pub certificate: Option<String>,

    /// VTTY rows for the terminal.
    ///
    /// Optional — defaults to the global `[vtty].rows` setting.
    #[serde(default)]
    pub rows: Option<u16>,

    /// VTTY columns for the terminal.
    ///
    /// Optional — defaults to the global `[vtty].cols` setting.
    #[serde(default)]
    pub cols: Option<u16>,

    /// Whether to retain the terminal buffer after the command exits.
    #[serde(default)]
    pub retain_on_exit: Option<bool>,
}

/// A single panel within an environment.
///
/// Each panel optionally connects to a server and spawns commands.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EnvironmentPanel {
    /// Panel title/label displayed in the panel header.
    ///
    /// Optional — defaults to the command name or "Panel N".
    #[serde(default)]
    pub title: Option<String>,

    /// Server URL for this panel's commands.
    ///
    /// Optional — if omitted, uses the environment's default server
    /// or the primary (local) instance. If set to a remote URL,
    /// commands are spawned on that remote instance.
    ///
    /// Example: `"http://localhost:9090"`, `"https://prod.example.com:9090"`
    #[serde(default)]
    pub server: Option<String>,

    /// Auth token for the server (if different from the global token).
    #[serde(default)]
    pub token: Option<String>,

    /// Label for the server connection (displayed in the sidebar).
    #[serde(default)]
    pub server_label: Option<String>,

    /// Commands to spawn in this panel.
    ///
    /// Optional — if empty, the panel is created without a running command.
    /// The user can manually connect a command later.
    #[serde(default)]
    pub commands: Vec<EnvironmentCommand>,
}

/// An environment configuration.
///
/// An environment defines a complete workspace setup: one or more panels,
/// each optionally connected to a server and running commands.
/// Environments allow users to quickly switch between different work
/// contexts (e.g., "Development", "Production Monitoring", "CI Pipeline").
///
/// Example TOML:
/// ```toml
/// [[environments]]
/// name = "Dev Workspace"
/// description = "Local development with frontend, backend, and database monitors"
/// layout = "horizontal"
/// auto_start = true
///
/// [[environments.panels]]
/// title = "Frontend"
/// commands = [{ cmd = "npm", args = "run dev", workdir = "/home/user/frontend" }]
///
/// [[environments.panels]]
/// title = "Backend"
/// commands = [{ cmd = "cargo", args = "run", workdir = "/home/user/api" }]
///
/// [[environments.panels]]
/// title = "Database"
/// server = "http://db-server:9090"
/// server_label = "DB Server"
/// commands = [{ cmd = "psql", args = "-U admin mydb" }]
/// ```
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WorkspaceEnvironment {
    /// Unique name for this environment.
    ///
    /// Used for display in the web UI and for CLI selection.
    pub name: String,

    /// Optional description of what this environment is for.
    #[serde(default)]
    pub description: Option<String>,

    /// Panel layout direction: "horizontal" (side-by-side) or "vertical" (stacked).
    ///
    /// Optional — defaults to "horizontal".
    #[serde(default)]
    pub layout: Option<String>,

    /// Whether to automatically start this environment when the server loads.
    ///
    /// If true, the server will pre-spawn all commands in all panels
    /// when the environment is activated.
    #[serde(default)]
    pub auto_start: Option<bool>,

    /// Default server URL for panels that don't specify their own.
    ///
    /// Optional — defaults to the primary (local) instance.
    #[serde(default)]
    pub default_server: Option<String>,

    /// Default auth token for panels that don't specify their own.
    #[serde(default)]
    pub default_token: Option<String>,

    /// The panels that make up this environment.
    #[serde(default)]
    pub panels: Vec<EnvironmentPanel>,
}

/// The environments section of the configuration.
///
/// Contains an array of `[[environments]]` entries.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct EnvironmentsConfig(pub Vec<WorkspaceEnvironment>);

impl EnvironmentsConfig {
    /// Iterate over the environment entries.
    pub fn iter(&self) -> impl Iterator<Item = &WorkspaceEnvironment> {
        self.0.iter()
    }

    /// Number of environments.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether there are no environments.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Find an environment by name (case-insensitive).
    pub fn find_by_name(&self, name: &str) -> Option<&WorkspaceEnvironment> {
        self.0.iter().find(|e| e.name.eq_ignore_ascii_case(name))
    }

    /// Get all auto-start environments.
    pub fn auto_start(&self) -> Vec<&WorkspaceEnvironment> {
        self.0.iter().filter(|e| e.auto_start == Some(true)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_env(name: &str) -> WorkspaceEnvironment {
        WorkspaceEnvironment {
            name: name.to_string(),
            description: Some(format!("{} env", name)),
            layout: Some("horizontal".to_string()),
            auto_start: Some(true),
            default_server: Some("http://localhost:9090".to_string()),
            default_token: Some("tok123".to_string()),
            panels: vec![
                EnvironmentPanel {
                    title: Some("Panel 1".to_string()),
                    server: Some("http://localhost:9090".to_string()),
                    token: Some("tok123".to_string()),
                    server_label: Some("local".to_string()),
                    commands: vec![EnvironmentCommand {
                        cmd: "bash".to_string(),
                        args: Some("-l".to_string()),
                        workdir: Some("/tmp".to_string()),
                        certificate: Some("cert1".to_string()),
                        rows: Some(30),
                        cols: Some(100),
                        retain_on_exit: Some(true),
                    }],
                },
            ],
        }
    }

    #[test]
    fn test_find_by_name_case_insensitive() {
        let cfg = EnvironmentsConfig(vec![make_env("Dev"), make_env("Prod")]);
        assert!(cfg.find_by_name("dev").is_some());
        assert!(cfg.find_by_name("DEV").is_some());
        assert!(cfg.find_by_name("Prod").is_some());
        assert!(cfg.find_by_name("prod").is_some());
        assert!(cfg.find_by_name("staging").is_none());
    }

    #[test]
    fn test_auto_start_filter() {
        let mut cfg = EnvironmentsConfig(vec![make_env("Dev"), make_env("Prod")]);
        assert_eq!(cfg.auto_start().len(), 2);
        cfg.0[1].auto_start = Some(false);
        assert_eq!(cfg.auto_start().len(), 1);
        assert_eq!(cfg.auto_start()[0].name, "Dev");
    }
}
