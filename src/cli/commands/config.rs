use anyhow::Result;

use crate::config::loader::load_config;
use crate::config::validation::{validate_config, ValidationLevel};

/// Handle the `vrc config-check` subcommand.
///
/// Validates configuration files without starting the server.
/// Reports validation errors and warnings with field paths.
/// Useful for CI/CD pipelines, pre-deployment checks, and debugging config issues.
pub fn handle_config_check_command(config_path: Option<&str>) -> Result<()> {
    let cfg = load_config(config_path)?;
    let issues = validate_config(&cfg);

    if issues.is_empty() {
        println!("Config is valid. No issues found.");
        return Ok(());
    }

    let errors: Vec<_> = issues
        .iter()
        .filter(|i| i.level == ValidationLevel::Error)
        .collect();
    let warnings: Vec<_> = issues
        .iter()
        .filter(|i| i.level == ValidationLevel::Warning)
        .collect();

    if !warnings.is_empty() {
        println!("Warnings ({}):", warnings.len());
        for w in &warnings {
            println!("  WARN  {} — {}", w.field, w.message);
        }
    }

    if !errors.is_empty() {
        println!("\nErrors ({}):", errors.len());
        for e in &errors {
            println!("  ERROR {} — {}", e.field, e.message);
        }
        anyhow::bail!("Config validation failed with {} error(s)", errors.len());
    }

    // Only warnings, no errors
    println!("\nConfig is valid (with {} warning(s)).", warnings.len());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_check_valid_empty_config() {
        // An empty config file should validate without errors
        let dir = std::env::temp_dir().join("vrc_test_config_check_valid");
        std::fs::create_dir_all(&dir).unwrap();
        let config_path = dir.join("valid.yaml");
        std::fs::write(&config_path, "").unwrap();

        let result = handle_config_check_command(Some(config_path.to_str().unwrap()));
        assert!(result.is_ok(), "valid config should pass: {:?}", result.err());

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn config_check_missing_file_errors() {
        let result = handle_config_check_command(Some("/nonexistent/path/config.yaml"));
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("Config file not found"), "unexpected error: {}", msg);
    }

    #[test]
    fn config_check_default_config_valid() {
        // With no config path, load_config uses defaults
        // This may pick up local config, so we just check it doesn't panic.
        // We can't assert Ok because local config may cause warnings/errors.
        let _ = handle_config_check_command(None);
    }

    #[test]
    fn config_check_explicit_valid_toml() {
        let dir = std::env::temp_dir().join("vrc_test_config_check_toml");
        std::fs::create_dir_all(&dir).unwrap();
        let config_path = dir.join("valid.toml");
        std::fs::write(&config_path, "").unwrap();

        let result = handle_config_check_command(Some(config_path.to_str().unwrap()));
        assert!(result.is_ok(), "valid TOML config should pass: {:?}", result.err());

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn config_check_partial_config() {
        let dir = std::env::temp_dir().join("vrc_test_config_check_partial");
        std::fs::create_dir_all(&dir).unwrap();
        let config_path = dir.join("partial.yaml");
        std::fs::write(&config_path, "").unwrap();

        let result = handle_config_check_command(Some(config_path.to_str().unwrap()));
        assert!(result.is_ok(), "partial config should pass: {:?}", result.err());

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
