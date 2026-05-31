//! CLI dispatch logic.
//!
//! `pre_runtime()` handles all subcommands synchronously before the tokio runtime.
//! `resolve_config()` loads, merges, and validates configuration.
//!
//! IPC subcommands (keys, cat, spawn-in, freeze, thaw, resize) require a minimal
//! tokio runtime for the async UDS client.

use anyhow::Result;
use std::io::stdout;

use clap::CommandFactory;

use crate::cli::args::{Cli, Commands};
use crate::cli::subcommands;
use crate::config::loader::load_config;
use crate::config::merge::apply_profile;
use crate::config::schema::Config;
use crate::config::validation::{validate_config, ValidationLevel};

/// Binary name used for completions.
#[cfg(feature = "vrunner")]
const BINARY_NAME: &str = "vrunner";
#[cfg(not(feature = "vrunner"))]
const BINARY_NAME: &str = "vrl";

/// Load, profile-merge, and CLI-override the configuration.
pub fn resolve_config(cli: &Cli) -> Result<Config> {
    let mut cfg = load_config(cli.config.as_deref())?;

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

    cli.apply_overrides(&mut cfg)?;

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

// ── vrl dispatch ──

/// Synchronous pre-runtime phase (vrl).
#[cfg(not(feature = "vrunner"))]
pub fn pre_runtime() -> Result<Option<Cli>> {
    let cli = Cli::parse_with_version();

    match &cli.command {
        Some(Commands::List) => {
            return Ok(Some(cli));
        }
        Some(Commands::Stop { pid }) => {
            tracing_subscriber::fmt::init();
            let registry = crate::instance::registry::InstanceRegistry::new()?;
            let instances = registry.list_instances();
            let resolved_pid = subcommands::resolve_stop_target(*pid, &instances);
            subcommands::handle_stop_command(Some(resolved_pid), &instances)?;
            return Ok(None);
        }
        Some(Commands::ConfigCheck) => {
            tracing_subscriber::fmt::init();
            subcommands::handle_config_check_command(cli.config.as_deref())?;
            return Ok(None);
        }
        Some(Commands::Completions { shell }) => {
            let mut cmd = <Cli as CommandFactory>::command();
            clap_complete::generate(*shell, &mut cmd, BINARY_NAME, &mut stdout());
            return Ok(None);
        }
        // IPC commands — handled by the caller after tokio runtime is available
        Some(Commands::Keys { .. })
        | Some(Commands::Cat { .. })
        | Some(Commands::SpawnIn { .. })
        | Some(Commands::Freeze { .. })
        | Some(Commands::Thaw { .. })
        | Some(Commands::Resize { .. })
        | Some(Commands::Kill { .. }) => {
            return Ok(Some(cli));
        }
        None => {}
    }

    Ok(Some(cli))
}

/// Check if the parsed CLI represents an IPC subcommand (vrl).
#[cfg(not(feature = "vrunner"))]
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
            | Some(Commands::Kill { .. })
    )
}

/// Dispatch an IPC subcommand (vrl).
#[cfg(not(feature = "vrunner"))]
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
        Some(Commands::Kill { pid, command }) => {
            tracing_subscriber::fmt::init();
            verify_instance(pid)?;
            subcommands::handle_kill_command(pid, command.as_deref()).await
        }
        _ => anyhow::bail!("Not an IPC command"),
    }
}

// ── vrunner dispatch ──

/// Synchronous pre-runtime phase (vrunner).
#[cfg(feature = "vrunner")]
pub fn pre_runtime() -> Result<Option<Cli>> {
    let cli = Cli::parse_with_version();

    match &cli.command {
        Some(Commands::List) => {}
        Some(Commands::Stop { pid: _ }) => {}
        Some(Commands::Spawn { .. }) => {}
        Some(Commands::Freeze { pid: _ }) => {}
        Some(Commands::Thaw { pid: _ }) => {}
        Some(Commands::Cert { action }) => {
            subcommands::handle_cert_command(action)?;
            return Ok(None);
        }
        Some(Commands::ListVrunner) => {}
        Some(Commands::ListCommands) => {}
        Some(Commands::StopCommand { target: _ }) => {}
        Some(Commands::Purge { target: _ }) => {}
        Some(Commands::Resize { .. }) => {}
        Some(Commands::ConfigCheck) => {
            subcommands::handle_config_check_command(cli.config.as_deref())?;
            return Ok(None);
        }
        Some(Commands::Cat {
            target: _,
            color_always: _,
        }) => {}
        Some(Commands::Screenshot { .. }) => {}
        Some(Commands::Completions { shell }) => {
            let mut cmd = <Cli as CommandFactory>::command();
            clap_complete::generate(*shell, &mut cmd, BINARY_NAME, &mut stdout());
            return Ok(None);
        }
        None => {}
    }

    Ok(Some(cli))
}

/// Dispatch async subcommands (vrunner). Returns true if handled.
#[cfg(feature = "vrunner")]
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
            subcommands::handle_freeze_command_http(cli, *pid).await?;
            Ok(true)
        }
        Some(Commands::Thaw { pid }) => {
            subcommands::handle_thaw_command_http(cli, *pid).await?;
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
                        "No matching exited command found for '{}'.", t
                    ),
                    None => tracing::error!(
                        "No exited command to purge."
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
            subcommands::handle_cat_command_http(cli, target.as_deref(), *color_always).await?;
            Ok(true)
        }
        Some(Commands::Screenshot {
            target,
            output,
            font_size,
            font_name,
        }) => {
            subcommands::handle_screenshot_command(
                cli,
                target.as_deref(),
                output.as_deref(),
                *font_size,
                font_name.as_deref(),
            )
            .await?;
            Ok(true)
        }
        Some(Commands::Cert { .. }) | Some(Commands::ConfigCheck) | Some(Commands::Completions { .. }) => unreachable!(),
        None => Ok(false),
    }
}
