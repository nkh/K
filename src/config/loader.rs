use anyhow::Result;
use config::{Config as ConfigBuilder, File, FileFormat};
use std::path::Path;

use super::merge::merge_configs;
use super::schema::Config;

/// Config directory name: "vrc" for vrc binary, "vrw" for vrw binary.
#[cfg(feature = "vrw")]
const APP_NAME: &str = "vrw";
#[cfg(not(feature = "vrw"))]
const APP_NAME: &str = "vrc";

/// Local config file name: "vrc.yaml" / "vrw.yaml".
#[cfg(feature = "vrw")]
const LOCAL_CONFIG: &str = "vrw";
#[cfg(not(feature = "vrw"))]
const LOCAL_CONFIG: &str = "vrc";

/// Try to detect the format of a config file from its extension.
fn detect_format(path: &Path) -> Option<FileFormat> {
    match path
        .extension()
        .and_then(|e| e.to_str())?
        .to_lowercase()
        .as_str()
    {
        "yaml" | "yml" => Some(FileFormat::Yaml),
        "toml" => Some(FileFormat::Toml),
        "json" => Some(FileFormat::Json),
        _ => None,
    }
}

pub fn load_config(cli_path: Option<&str>) -> Result<Config> {
    let mut builder = ConfigBuilder::builder();

    // Global config — try YAML first, then TOML
    if let Some(global_dir) = dirs::config_dir() {
        let global_yaml = global_dir.join(APP_NAME).join("config.yaml");
        let global_toml = global_dir.join(APP_NAME).join("config.toml");

        if global_yaml.exists() {
            let fmt = detect_format(&global_yaml).unwrap_or(FileFormat::Yaml);
            builder = builder.add_source(File::from(global_yaml.as_path()).format(fmt));
        } else if global_toml.exists() {
            let fmt = detect_format(&global_toml).unwrap_or(FileFormat::Toml);
            builder = builder.add_source(File::from(global_toml.as_path()).format(fmt));
        }
    }

    // Local config — try YAML first, then TOML
    let cwd = std::env::current_dir()?;
    let local_yaml = cwd.join(format!("{}.yaml", LOCAL_CONFIG));
    let local_toml = cwd.join(format!("{}.toml", LOCAL_CONFIG));
    let mut local_path: Option<std::path::PathBuf> = None;

    if local_yaml.exists() {
        let fmt = detect_format(&local_yaml).unwrap_or(FileFormat::Yaml);
        builder = builder.add_source(File::from(local_yaml.as_path()).format(fmt));
        local_path = Some(local_yaml);
    } else if local_toml.exists() {
        let fmt = detect_format(&local_toml).unwrap_or(FileFormat::Toml);
        builder = builder.add_source(File::from(local_toml.as_path()).format(fmt));
        local_path = Some(local_toml);
    }

    // CLI-specified config
    if let Some(path) = cli_path {
        let p = Path::new(path);
        if p.exists() {
            let fmt = detect_format(p).unwrap_or(FileFormat::Yaml);
            builder = builder.add_source(File::from(p).format(fmt));
        } else {
            anyhow::bail!("Config file not found: {}", path);
        }
    }

    let settings = builder.build()?;
    let mut global: Config = settings.try_deserialize()?;

    // If local config exists, merge it over global
    if let Some(ref lpath) = local_path {
        let fmt = detect_format(lpath).unwrap_or(FileFormat::Yaml);
        let local_settings = ConfigBuilder::builder()
            .add_source(File::from(lpath.as_path()).format(fmt))
            .build()?;
        let local: Config = local_settings.try_deserialize()?;
        global = merge_configs(global, local);
    }

    Ok(global)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_format_yaml() {
        assert_eq!(
            detect_format(Path::new("config.yaml")),
            Some(FileFormat::Yaml)
        );
    }

    #[test]
    fn detect_format_yml() {
        assert_eq!(
            detect_format(Path::new("config.yml")),
            Some(FileFormat::Yaml)
        );
    }

    #[test]
    fn detect_format_toml() {
        assert_eq!(
            detect_format(Path::new("config.toml")),
            Some(FileFormat::Toml)
        );
    }

    #[test]
    fn detect_format_json() {
        assert_eq!(
            detect_format(Path::new("config.json")),
            Some(FileFormat::Json)
        );
    }

    #[test]
    fn detect_format_unknown_extension() {
        assert_eq!(detect_format(Path::new("config.xml")), None);
    }

    #[test]
    fn detect_format_no_extension() {
        assert_eq!(detect_format(Path::new("config")), None);
    }

    #[test]
    fn detect_format_hidden_file_no_ext() {
        assert_eq!(detect_format(Path::new(".config")), None);
    }

    #[test]
    fn detect_format_empty_string() {
        assert_eq!(detect_format(Path::new("")), None);
    }

    #[test]
    fn detect_format_case_insensitive() {
        assert_eq!(
            detect_format(Path::new("CONFIG.YAML")),
            Some(FileFormat::Yaml)
        );
        assert_eq!(
            detect_format(Path::new("CONFIG.TOML")),
            Some(FileFormat::Toml)
        );
        assert_eq!(
            detect_format(Path::new("CONFIG.JSON")),
            Some(FileFormat::Json)
        );
    }

    #[test]
    fn detect_format_with_directory_path() {
        assert_eq!(
            detect_format(Path::new("/home/user/.config/vrc/config.yaml")),
            Some(FileFormat::Yaml)
        );
        assert_eq!(
            detect_format(Path::new("/etc/vrc/config.toml")),
            Some(FileFormat::Toml)
        );
    }

    #[test]
    fn load_config_with_explicit_valid_yaml() {
        let dir = std::env::temp_dir().join("vrc_test_load_yaml");
        std::fs::create_dir_all(&dir).unwrap();
        let config_path = dir.join("test.yaml");
        std::fs::write(
            &config_path,
            "vtty:\n  rows: 50\n  cols: 200\n  term: \"xterm-256color\"\n  scrollback: 10000\n  truecolor: true\n  mouse: false\n",
        )
        .unwrap();

        let config = load_config(Some(config_path.to_str().unwrap())).unwrap();
        assert_eq!(config.vtty.rows, 50);
        assert_eq!(config.vtty.cols, 200);
        assert_eq!(config.vtty.term, "xterm-256color");
        assert_eq!(config.vtty.scrollback, 10000);
        assert!(config.vtty.truecolor);
        assert!(!config.vtty.mouse);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn load_config_with_explicit_valid_toml() {
        let dir = std::env::temp_dir().join("vrc_test_load_toml");
        std::fs::create_dir_all(&dir).unwrap();
        let config_path = dir.join("test.toml");
        std::fs::write(
            &config_path,
            "[vtty]\nrows = 30\ncols = 120\nterm = \"xterm-256color\"\nscrollback = 2000\ntruecolor = true\nmouse = false\n",
        )
        .unwrap();

        let config = load_config(Some(config_path.to_str().unwrap())).unwrap();
        assert_eq!(config.vtty.rows, 30);
        assert_eq!(config.vtty.cols, 120);
        assert_eq!(config.vtty.scrollback, 2000);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn load_config_with_explicit_valid_json() {
        let dir = std::env::temp_dir().join("vrc_test_load_json");
        std::fs::create_dir_all(&dir).unwrap();
        let config_path = dir.join("test.json");
        std::fs::write(
            &config_path,
            r#"{"vtty": {"rows": 80, "cols": 240, "term": "xterm-256color", "scrollback": 5000, "truecolor": true, "mouse": false}}"#,
        )
        .unwrap();

        let config = load_config(Some(config_path.to_str().unwrap())).unwrap();
        assert_eq!(config.vtty.rows, 80);
        assert_eq!(config.vtty.cols, 240);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn load_config_missing_explicit_path_errors() {
        let result = load_config(Some("/nonexistent/path/config.yaml"));
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Config file not found"));
    }

    #[test]
    fn load_config_with_empty_config_file_uses_defaults() {
        let dir = std::env::temp_dir().join("vrc_test_empty_config");
        std::fs::create_dir_all(&dir).unwrap();
        let config_path = dir.join("empty.yaml");
        std::fs::write(&config_path, "").unwrap();

        let config = load_config(Some(config_path.to_str().unwrap())).unwrap();
        // Should have built-in defaults
        assert_eq!(config.vtty.rows, 24);
        assert_eq!(config.vtty.cols, 80);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn load_config_with_partial_config_uses_partial_defaults() {
        let dir = std::env::temp_dir().join("vrc_test_partial_config");
        std::fs::create_dir_all(&dir).unwrap();
        let config_path = dir.join("partial.yaml");
        std::fs::write(&config_path, "vtty:\n  rows: 100\n  cols: 80\n  term: \"xterm-256color\"\n  scrollback: 5000\n  truecolor: true\n  mouse: false\n").unwrap();

        let config = load_config(Some(config_path.to_str().unwrap())).unwrap();
        assert_eq!(config.vtty.rows, 100);
        // cols should still be what we set (same as default)
        assert_eq!(config.vtty.cols, 80);

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
