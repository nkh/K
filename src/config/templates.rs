use serde::{Deserialize, Serialize};

/// A pre-defined command template.
///
/// Templates appear in the web UI's Templates sidebar tab and allow
/// users to spawn frequently-used commands with a single click.
/// Optional arguments and environment variables are pre-filled but
/// can be overridden at spawn time.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TemplateConfig {
    /// Display name shown in the Templates panel.
    ///
    /// Example: `"Dev server"`
    pub name: String,

    /// The command executable to run.
    ///
    /// Example: `"npm"`
    /// Example: `"/usr/bin/htop"`
    pub cmd: String,

    /// Space-separated arguments passed to the command.
    ///
    /// Example: `"run dev"`
    /// Optional — omit or leave empty for no arguments.
    #[serde(default)]
    pub args: Option<String>,

    /// Environment variables to set when spawning this template.
    ///
    /// Each entry is a `KEY=VALUE` string.  These override the global
    /// `[environment]` defaults but can be overridden per-spawn via the
    /// API `env` field.
    ///
    /// Optional — omit or leave empty for no extra environment.
    #[serde(default)]
    pub env: Option<Vec<String>>,

    /// Working directory for the spawned command.
    ///
    /// Optional — defaults to vrc's own working directory.
    #[serde(default)]
    pub workdir: Option<String>,

    /// Certificate name to bind (from the `[certificates]` section).
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
}

/// The templates section of the configuration.
///
/// Contains an array of `[[templates]]` entries.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct TemplatesConfig(pub Vec<TemplateConfig>);

impl TemplatesConfig {
    /// Iterate over the template entries.
    pub fn iter(&self) -> impl Iterator<Item = &TemplateConfig> {
        self.0.iter()
    }

    /// Number of templates.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether there are no templates.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}


