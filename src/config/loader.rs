use anyhow::Result;
use config::{Config as ConfigBuilder, File, FileFormat};
use std::path::Path;

use super::merge::merge_configs;
use super::schema::Config;

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
        let global_yaml = global_dir.join("vrunner").join("config.yaml");
        let global_toml = global_dir.join("vrunner").join("config.toml");

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
    let local_yaml = cwd.join("vrunner.yaml");
    let local_toml = cwd.join("vrunner.toml");
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
