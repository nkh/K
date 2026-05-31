//! CLI dispatch logic.
//!
//! `pre_runtime()` handles all subcommands synchronously before the tokio runtime.
//! `resolve_config()` loads, merges, and validates configuration.
//!
//! IPC subcommands (keys, cat, spawn-in, freeze, thaw, resize) require a minimal
//! tokio runtime for the async UDS client.  These are handled in `main()` after
//! `pre_runtime()` returns `Ok(Some(cli))` — but we detect them here and return
//! a special sentinel so the caller knows to run the IPC handler instead of
//! starting a full vrunner instance.

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
/// Synchronous subcommands (list, stop, config-check, completions) are handled
/// here directly.  IPC subcommands (keys, cat, spawn-in, freeze, thaw, resize)
/// are detected but deferred — they return Ok(Some(cli)) and the caller checks
/// `is_ipc_command()` to decide whether to run the IPC handler or start a
/// full vrunner instance.
pub fn pre_runtime() -> Result<Option<Cli>> {
    let cli = Cli::parse_with_version();

    // Handle subcommands
    match &cli.command {
        // List — needs UDS to query commands from each instance
        Some(Commands::List) => {
            return Ok(Some(cli));
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
        // IPC commands — handled by the caller after tokio runtime is available
        Some(Commands::Keys { .. })
        | Some(Commands::Cat { .. })
        | Some(Commands::SpawnIn { .. })
        | Some(Commands::Freeze { .. })
        | Some(Commands::Thaw { .. })
        | Some(Commands::Resize { .. }) => {
            return Ok(Some(cli));
        }
        None => {}
    }

    Ok(Some(cli))
}

/// Check if the parsed CLI represents an IPC subcommand (one that needs
/// a UDS connection to a running instance, not a new vrunner instance).
pub fn is_ipc_command(cli: &Cli) -> bool {
    matches!(
        cli.command,
        Some(Commands::List)
            | Some(Commands::Keys { .. })
            | Some(Commands::Cat { .. })
            | Some(Commands::SpawnIn { .. })
            | Some(Commands::Freeze { .. })
            | Some(Commands::Thaw { .. })
            | Some(Commands::Resize { .. })
    )
}

/// Dispatch an IPC subcommand.  Requires a tokio runtime.
pub async fn run_ipc_command(cli: Cli) -> Result<()> {
    use crate::cli::commands::verify_instance;

    match cli.command {
        Some(Commands::List) => {
            tracing_subscriber::fmt::init();
            subcommands::handle_list_command(&cli).await
        }
        Some(Commands::Keys { pid, command, keys }) => {
            tracing_subscriber::fmt::init();
            verify_instance(pid)?;
            subcommands::handle_keys_command(pid, command.as_deref(), &keys).await
        }
        Some(Commands::Cat { pid, command }) => {
            tracing_subscriber::fmt::init();
            verify_instance(pid)?;
            subcommands::handle_cat_command(pid, command.as_deref()).await
        }
        Some(Commands::SpawnIn { pid, cmd, args }) => {
            tracing_subscriber::fmt::init();
            verify_instance(pid)?;
            subcommands::handle_spawn_in_command(pid, &cmd, &args).await
        }
        Some(Commands::Freeze { pid, command }) => {
            tracing_subscriber::fmt::init();
            verify_instance(pid)?;
            subcommands::handle_freeze_command(pid, command.as_deref()).await
        }
        Some(Commands::Thaw { pid, command }) => {
            tracing_subscriber::fmt::init();
            verify_instance(pid)?;
            subcommands::handle_thaw_command(pid, command.as_deref()).await
        }
        Some(Commands::Resize {
            pid,
            command,
            rows,
            cols,
        }) => {
            tracing_subscriber::fmt::init();
            verify_instance(pid)?;
            subcommands::handle_resize_command(pid, command.as_deref(), rows, cols).await
        }
        _ => anyhow::bail!("Not an IPC command"),
    }
}
