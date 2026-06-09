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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_template(name: &str, cmd: &str) -> TemplateConfig {
        TemplateConfig {
            name: name.to_string(),
            cmd: cmd.to_string(),
            args: Some("arg1 arg2".to_string()),
            env: Some(vec!["KEY=VAL".to_string()]),
            workdir: Some("/tmp".to_string()),
            certificate: Some("cert1".to_string()),
            rows: Some(40),
            cols: Some(120),
        }
    }

    #[test]
    fn test_templates_config_default_empty() {
        let cfg = TemplatesConfig::default();
        assert!(cfg.is_empty());
        assert_eq!(cfg.len(), 0);
        assert_eq!(cfg.iter().count(), 0);
    }

    #[test]
    fn test_templates_config_iter() {
        let cfg = TemplatesConfig(vec![
            make_template("Dev", "npm"),
            make_template("Prod", "cargo"),
        ]);
        assert_eq!(cfg.len(), 2);
        assert!(!cfg.is_empty());
        assert_eq!(cfg.iter().count(), 2);
    }

    #[test]
    fn test_template_config_fields() {
        let t = make_template("Dev Server", "npm");
        assert_eq!(t.name, "Dev Server");
        assert_eq!(t.cmd, "npm");
        assert_eq!(t.args, Some("arg1 arg2".to_string()));
        assert_eq!(t.env, Some(vec!["KEY=VAL".to_string()]));
        assert_eq!(t.workdir, Some("/tmp".to_string()));
        assert_eq!(t.certificate, Some("cert1".to_string()));
        assert_eq!(t.rows, Some(40));
        assert_eq!(t.cols, Some(120));
    }

    #[test]
    fn test_template_config_minimal() {
        let t = TemplateConfig {
            name: "htop".to_string(),
            cmd: "htop".to_string(),
            args: None,
            env: None,
            workdir: None,
            certificate: None,
            rows: None,
            cols: None,
        };
        assert!(t.args.is_none());
        assert!(t.env.is_none());
        assert!(t.workdir.is_none());
        assert!(t.certificate.is_none());
        assert!(t.rows.is_none());
        assert!(t.cols.is_none());
    }

    #[test]
    fn test_template_config_serialization_roundtrip() {
        let t = make_template("Dev", "npm");
        let json = serde_json::to_string(&t).unwrap();
        let deserialized: TemplateConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, t.name);
        assert_eq!(deserialized.cmd, t.cmd);
        assert_eq!(deserialized.args, t.args);
        assert_eq!(deserialized.env, t.env);
    }

    #[test]
    fn test_templates_config_serialization() {
        let cfg = TemplatesConfig(vec![make_template("Dev", "npm")]);
        let json = serde_json::to_string(&cfg).unwrap();
        let deserialized: TemplatesConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.len(), 1);
        assert_eq!(deserialized.0[0].name, "Dev");
    }
}
