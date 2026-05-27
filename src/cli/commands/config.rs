use anyhow::Result;

use crate::config::loader::load_config;
use crate::config::validation::{validate_config, ValidationLevel};

/// Handle the `vrunner config-check` subcommand.
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
