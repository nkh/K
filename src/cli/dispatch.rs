//! CLI dispatch logic — synchronous pre-runtime and async subcommand routing.
//!
//! `pre_runtime()` handles subcommands that don't need the tokio runtime
//! (cert, config-check) and falls through for everything else.
//! `handle_subcommands()` dispatches async subcommands (list, stop, spawn,
//! freeze, thaw, cat, resize, purge) and returns `true` if one was handled.

use anyhow::Result;

use crate::cli::args::{Cli, Commands};
use crate::cli::subcommands;
use crate::config::loader::load_config;
use crate::config::merge::apply_profile;
use crate::config::schema::Config;
use crate::config::validation::{validate_config, ValidationLevel};

/// Load, profile-merge, and CLI-override the configuration.
///
/// Config loading is intentionally **lazy** — it only runs after clap has
/// finished parsing, so `vrunner --help` and `vrunner --version` respond
/// instantly without touching the filesystem.  Subcommands that don't
/// need config (list, stop, spawn, freeze, thaw, etc.) also return
/// before reaching this code path.
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
/// and daemonize. Daemonization MUST happen before the tokio runtime starts,
/// because fork() only copies the calling thread while tokio's multi-threaded
/// runtime creates internal threads for I/O, timers, and blocking tasks.
pub fn pre_runtime() -> Result<Option<Cli>> {
    let cli = Cli::parse_with_version();

    // Handle subcommands that don't need the runtime
    match &cli.command {
        Some(Commands::List) => {
            // list is async (needs to query instances for their commands), fall through
        }
        Some(Commands::Stop { pid: _ }) => {
            // stop_instance is async (uses reqwest), so we need the runtime
        }
        Some(Commands::Spawn { .. }) => {
            // spawn is async (uses reqwest), fall through to async phase
        }
        Some(Commands::Freeze { pid: _ }) => {
            // freeze is async (uses reqwest), fall through to async phase
        }
        Some(Commands::Thaw { pid: _ }) => {
            // thaw is async (uses reqwest), fall through to async phase
        }
        Some(Commands::Cert { action }) => {
            // Cert subcommands are synchronous — handle them here
            subcommands::handle_cert_command(action)?;
            return Ok(None);
        }
        Some(Commands::ListVrunner) => {
            // list-vrunner is async (needs HTTP), fall through to async phase
        }
        Some(Commands::ListCommands) => {
            // list-commands is async (needs HTTP), fall through to async phase
        }
        Some(Commands::StopCommand { target: _ }) => {
            // stop-command is async (needs HTTP), fall through to async phase
        }
        Some(Commands::Purge { target: _ }) => {
            // purge is async (needs HTTP), fall through to async phase
        }
        Some(Commands::Resize { .. }) => {
            // resize is async (needs HTTP), fall through to async phase
        }
        Some(Commands::ConfigCheck) => {
            subcommands::handle_config_check_command(cli.config.as_deref())?;
            return Ok(None);
        }
        Some(Commands::Cat {
            target: _,
            color_always: _,
        }) => {
            // cat is async (needs HTTP), fall through to async phase
        }
        None => {}
    }

    Ok(Some(cli))
}

/// Dispatch async subcommands.  Returns `Ok(true)` if a subcommand was
/// handled (the caller should return), or `Ok(false)` if no subcommand
/// matched (the caller should continue to the main server path).
pub async fn handle_subcommands(cli: &Cli) -> Result<bool> {
    match &cli.command {
        Some(Commands::List) => {
            subcommands::handle_list_command(cli).await?;
            Ok(true)
        }
        Some(Commands::Stop { pid }) => {
            let registry = crate::instance::registry::InstanceRegistry::new()?;
            let instances = registry.list_instances();
            let pid = subcommands::resolve_stop_target(*pid, &instances);
            tracing::info!(instance_pid = pid, "Stopping vrunner instance");
            registry.stop_instance(pid).await?;
            Ok(true)
        }
        Some(Commands::Spawn {
            cmd,
            args,
            rows,
            cols,
        }) => {
            subcommands::handle_spawn_command(cli, cmd, args, *rows, *cols).await?;
            Ok(true)
        }
        Some(Commands::Freeze { pid }) => {
            subcommands::handle_freeze_command(cli, *pid).await?;
            Ok(true)
        }
        Some(Commands::Thaw { pid }) => {
            subcommands::handle_thaw_command(cli, *pid).await?;
            Ok(true)
        }
        Some(Commands::ListVrunner) => {
            subcommands::handle_list_vrunner_command(cli).await?;
            Ok(true)
        }
        Some(Commands::ListCommands) => {
            subcommands::handle_list_commands_command(cli).await?;
            Ok(true)
        }
        Some(Commands::StopCommand { target }) => {
            let stopped = subcommands::handle_stop_command(cli, target.as_deref()).await?;
            if !stopped {
                match target {
                    Some(t) => tracing::error!(
                        "No matching command found for '{}'. Use `vrunner list` to see running commands.", t
                    ),
                    None => tracing::error!(
                        "No command to stop. Use `vrunner list` to see running commands."
                    ),
                }
                std::process::exit(1);
            }
            Ok(true)
        }
        Some(Commands::Purge { target }) => {
            let purged = subcommands::handle_purge_command(cli, target.as_deref()).await?;
            if !purged {
                match target {
                    Some(t) => tracing::error!(
                        "No matching exited command found for '{}'. Use `vrunner list` to see commands.", t
                    ),
                    None => tracing::error!(
                        "No exited command to purge. Use `vrunner list` to see commands."
                    ),
                }
                std::process::exit(1);
            }
            Ok(true)
        }
        Some(Commands::Resize { target, rows, cols }) => {
            subcommands::handle_resize_command(cli, target, *rows, *cols).await?;
            Ok(true)
        }
        Some(Commands::Cat {
            target,
            color_always,
        }) => {
            subcommands::handle_cat_command(cli, target.as_deref(), *color_always).await?;
            Ok(true)
        }
        // Cert and ConfigCheck are handled synchronously in pre_runtime()
        // and never reach the async phase.
        Some(Commands::Cert { .. }) | Some(Commands::ConfigCheck) => unreachable!(),
        None => Ok(false),
    }
}
