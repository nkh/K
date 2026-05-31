//! CLI dispatch logic.
//!
//! `pre_runtime()` handles all subcommands synchronously before the tokio runtime.
//! `resolve_config()` loads, merges, and validates configuration.

use anyhow::Result;
use std::io::stdout;

use clap::CommandFactory;

use crate::cli::args::{Cli, Commands};
use crate::cli::subcommands;
use crate::config::loader::load_config;
use crate::config::merge::apply_profile;
use crate::config::schema::Config;
use crate::config::validation::{validate_config, ValidationLevel};
use crate::instance::registry::InstanceRegistry;

/// Load, profile-merge, and CLI-override the configuration.
pub fn resolve_config(cli: &Cli) -> Result<Config> {
    let mut cfg = load_config(cli.config.as_deref())?;

    // Apply named profile if specified
    if let Some(ref profile_name) = cli.profile {
        if let Some(profile) = cfg.profiles.entries.clone().get(profile_name) {
            tracing::info!(profile = %profile_name, "Applying configuration profile");
            cfg = apply_profile(cfg, profile);
        } else {
            anyhow::bail!(
                "Profile '{}' not found. Available profiles: {}",
                profile_name,
                if cfg.profiles.entries.is_empty() {
                    "(none defined in config)".to_string()
                } else {
                    cfg.profiles
                        .entries
                        .keys()
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(", ")
                }
            );
        }
    }

    // Apply CLI overrides (highest precedence)
    cli.apply_overrides(&mut cfg)?;

    // Validate the final merged configuration
    let issues = validate_config(&cfg);
    let mut error_fields = Vec::new();
    for issue in &issues {
        match issue.level {
            ValidationLevel::Error => {
                tracing::error!(field = %issue.field, "{}", issue.message);
                error_fields.push(issue.field.clone());
            }
            ValidationLevel::Warning => {
                tracing::warn!(field = %issue.field, "{}", issue.message);
            }
        }
    }
    if !error_fields.is_empty() {
        anyhow::bail!(
            "Configuration validation failed ({})
Fields with errors: {}",
            error_fields.len(),
            error_fields.join(", "),
        );
    }

    Ok(cfg)
}

/// Synchronous pre-runtime phase: parse CLI, handle subcommands, load config,
/// and daemonize.
///
/// All subcommands are handled synchronously here — no tokio runtime needed.
/// Returns Ok(Some(cli)) if the caller should start a vrunner instance,
/// or Ok(None) if a subcommand was handled and the process should exit.
pub fn pre_runtime() -> Result<Option<Cli>> {
    let cli = Cli::parse_with_version();

    // Handle subcommands
    match &cli.command {
        Some(Commands::List) => {
            // list is synchronous (reads PID files only)
            // Init tracing briefly for this subcommand
            tracing_subscriber::fmt::init();
            subcommands::handle_list_command(&cli)?;
            return Ok(None);
        }
        Some(Commands::Stop { pid }) => {
            // stop is synchronous (sends signal directly)
            tracing_subscriber::fmt::init();
            let registry = InstanceRegistry::new()?;
            let instances = registry.list_instances();
            let resolved_pid = subcommands::resolve_stop_target(*pid, &instances);
            subcommands::handle_stop_command(Some(resolved_pid), &instances)?;
            return Ok(None);
        }
        Some(Commands::ConfigCheck) => {
            // Init tracing for error reporting
            tracing_subscriber::fmt::init();
            subcommands::handle_config_check_command(cli.config.as_deref())?;
            return Ok(None);
        }
        Some(Commands::Completions { shell }) => {
            let mut cmd = <Cli as CommandFactory>::command();
            clap_complete::generate(*shell, &mut cmd, "vrunner", &mut stdout());
            return Ok(None);
        }
        None => {}
    }

    Ok(Some(cli))
}
