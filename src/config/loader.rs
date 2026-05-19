use anyhow::Result;
use config::{Config as ConfigBuilder, File, FileFormat};
use std::path::Path;

use super::schema::Config;
use super::merge::merge_configs;

pub fn load_config(cli_path: Option<&str>) -> Result<Config> {
    let mut builder = ConfigBuilder::builder();

    // Global config
    if let Some(global_dir) = dirs::config_dir() {
        let global_path = global_dir.join("vrunner").join("config.yaml");
        if global_path.exists() {
            builder = builder.add_source(File::from(global_path).format(FileFormat::Yaml));
        }
    }

    // Local config
    let local_path = std::env::current_dir()?.join("vrunner.yaml");
    if local_path.exists() {
        builder = builder.add_source(File::from(local_path).format(FileFormat::Yaml));
    }

    // CLI-specified config
    if let Some(path) = cli_path {
        builder = builder.add_source(File::from(Path::new(path)).format(FileFormat::Yaml));
    }

    let settings = builder.build()?;
    let mut global: Config = settings.try_deserialize()?;

    // If local config exists, merge it over global
    let local_path = std::env::current_dir()?.join("vrunner.yaml");
    if local_path.exists() {
        let local_settings = ConfigBuilder::builder()
            .add_source(File::from(local_path).format(FileFormat::Yaml))
            .build()?;
        let local: Config = local_settings.try_deserialize()?;
        global = merge_configs(global, local);
    }

    Ok(global)
}
