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

use crate::cli::args::{BINARY_NAME, Cli, Commands};
use crate::cli::subcommands;
use crate::config::loader::load_config;
use crate::config::merge::apply_profile;
use crate::config::schema::Config;
use crate::config::validation::{validate_config, ValidationLevel};

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

// ── vrc dispatch ──

/// Synchronous pre-runtime phase (vrc).
#[cfg(not(feature = "vrw"))]
pub fn pre_runtime() -> Result<Option<Cli>> {
    let cli = Cli::parse_with_version();

    match &cli.command {
        Some(Commands::List { interactive: _ }) => {
            return Ok(Some(cli));
        }
        Some(Commands::Stop { pid, interactive }) => {
            tracing_subscriber::fmt::init();
            let registry = crate::instance::registry::InstanceRegistry::new()?;
            let instances = registry.list_instances();

            // Interactive mode: let user select which instance(s) to stop
            if *interactive && pid.is_none() {
                if instances.is_empty() {
                    eprintln!("No vrc instances running.");
                    std::process::exit(1);
                }
                let items: Vec<_> = instances.iter().map(|i| {
                    crate::cli::interactive_select::SelectItem {
                        label: format!("PID {}", i.pid),
                        id: i.pid.to_string(),
                    }
                }).collect();
                let selected = crate::cli::interactive_select::select_items(
                    &items,
                    "Select instances to stop [space-separated numbers]",
                )?;
                for item in &selected {
                    let target_pid: u32 = item.id.parse().unwrap();
                    if let Err(e) = subcommands::handle_stop_command(Some(target_pid), &instances) {
                        eprintln!("Failed to stop instance {}: {}", target_pid, e);
                    }
                }
                std::process::exit(0);
            }

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

/// Check if the parsed CLI represents an IPC subcommand (vrc).
#[cfg(not(feature = "vrw"))]
pub fn is_ipc_command(cli: &Cli) -> bool {
    matches!(
        cli.command,
        Some(Commands::List { .. })
            | Some(Commands::Keys { .. })
            | Some(Commands::Cat { .. })
            | Some(Commands::SpawnIn { .. })
            | Some(Commands::Freeze { .. })
            | Some(Commands::Thaw { .. })
            | Some(Commands::Resize { .. })
            | Some(Commands::Kill { .. })
    )
}

/// Dispatch an IPC subcommand (vrc).
#[cfg(not(feature = "vrw"))]
pub async fn run_ipc_command(cli: Cli) -> Result<()> {
    use crate::cli::commands::verify_instance;

    match cli.command {
        Some(Commands::List { .. }) => {
            tracing_subscriber::fmt::init();
            subcommands::handle_list_command(&cli).await
        }
        Some(Commands::Keys { pid, command, keys }) => {
            tracing_subscriber::fmt::init();
            verify_instance(pid)?;
            subcommands::handle_keys_command(pid, command.as_deref(), &keys).await
        }
        Some(Commands::Cat { pid, command, plain, interactive }) => {
            tracing_subscriber::fmt::init();
            verify_instance(pid)?;
            if interactive {
                subcommands::handle_cat_command_interactive(pid, plain).await
            } else {
                subcommands::handle_cat_command(pid, command.as_deref(), plain).await
            }
        }
        Some(Commands::SpawnIn { pid, cmd, args }) => {
            tracing_subscriber::fmt::init();
            verify_instance(pid)?;
            subcommands::handle_spawn_in_command(pid, &cmd, &args).await
        }
        Some(Commands::Freeze { pid, command, interactive }) => {
            tracing_subscriber::fmt::init();
            verify_instance(pid)?;
            if interactive {
                subcommands::handle_freeze_command_interactive(pid).await
            } else {
                subcommands::handle_freeze_command(pid, command.as_deref()).await
            }
        }
        Some(Commands::Thaw { pid, command, interactive }) => {
            tracing_subscriber::fmt::init();
            verify_instance(pid)?;
            if interactive {
                subcommands::handle_thaw_command_interactive(pid).await
            } else {
                subcommands::handle_thaw_command(pid, command.as_deref()).await
            }
        }
        Some(Commands::Resize {
            pid,
            command,
            rows,
            cols,
            interactive,
        }) => {
            tracing_subscriber::fmt::init();
            verify_instance(pid)?;
            if interactive {
                subcommands::handle_resize_command_interactive(pid, rows, cols).await
            } else {
                subcommands::handle_resize_command(pid, command.as_deref(), rows, cols).await
            }
        }
        Some(Commands::Kill { pid, command, interactive }) => {
            tracing_subscriber::fmt::init();
            verify_instance(pid)?;
            if interactive {
                subcommands::handle_kill_command_interactive(pid).await
            } else {
                subcommands::handle_kill_command(pid, command.as_deref()).await
            }
        }
        _ => anyhow::bail!("Not an IPC command"),
    }
}

// ── vrw dispatch ──

/// Synchronous pre-runtime phase (vrw).
#[cfg(feature = "vrw")]
pub fn pre_runtime() -> Result<Option<Cli>> {
    let cli = Cli::parse_with_version();

    match &cli.command {
        Some(Commands::List { interactive: _ }) => {}
        Some(Commands::Stop { pid: _, interactive: _ }) => {}
        Some(Commands::Spawn { .. }) => {}
        Some(Commands::Freeze { pid: _, interactive: _ }) => {}
        Some(Commands::Thaw { pid: _, interactive: _ }) => {}
        Some(Commands::Cert { action }) => {
            subcommands::handle_cert_command(action)?;
            return Ok(None);
        }
        Some(Commands::ListVrw) => {}
        Some(Commands::ListCommands) => {}
        Some(Commands::StopCommand { target: _, interactive: _ }) => {}
        Some(Commands::Purge { target: _ }) => {}
        Some(Commands::Resize { target: _, rows: _, cols: _, interactive: _ }) => {}
        Some(Commands::ConfigCheck) => {
            subcommands::handle_config_check_command(cli.config.as_deref())?;
            return Ok(None);
        }
        Some(Commands::Cat {
            target: _,
            plain: _,
            interactive: _,
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

/// Dispatch async subcommands (vrw). Returns true if handled.
#[cfg(feature = "vrw")]
pub async fn handle_subcommands(cli: &Cli) -> Result<bool> {
    match &cli.command {
        Some(Commands::List { .. }) => {
            subcommands::handle_list_command(cli).await?;
            Ok(true)
        }
        Some(Commands::Stop { pid, interactive }) => {
            let registry = crate::instance::registry::InstanceRegistry::new()?;
            let instances = registry.list_instances();

            // Interactive mode: let user select which instance(s) to stop
            if *interactive && pid.is_none() {
                if instances.is_empty() {
                    tracing::error!("No vrw instances running.");
                    std::process::exit(1);
                }
                let items: Vec<_> = instances.iter().map(|i| {
                    crate::cli::interactive_select::SelectItem {
                        label: format!("PID {} — port {}", i.pid, i.port),
                        id: i.pid.to_string(),
                    }
                }).collect();
                let selected = crate::cli::interactive_select::select_items(
                    &items,
                    "Select instances to stop [space-separated numbers]",
                )?;
                for item in &selected {
                    let target_pid: u32 = item.id.parse().unwrap();
                    if let Err(e) = registry.stop_instance(target_pid).await {
                        tracing::error!("Failed to stop instance {}: {}", target_pid, e);
                    }
                }
                return Ok(true);
            }

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
        Some(Commands::Freeze { pid, interactive }) => {
            subcommands::handle_freeze_command_http(cli, *pid, *interactive).await?;
            Ok(true)
        }
        Some(Commands::Thaw { pid, interactive }) => {
            subcommands::handle_thaw_command_http(cli, *pid, *interactive).await?;
            Ok(true)
        }
        Some(Commands::ListVrw) => {
            subcommands::handle_list_vrw_command(cli).await?;
            Ok(true)
        }
        Some(Commands::ListCommands) => {
            subcommands::handle_list_commands_command(cli).await?;
            Ok(true)
        }
        Some(Commands::StopCommand { target, interactive }) => {
            let stopped = subcommands::handle_stop_command(cli, target.as_deref(), *interactive).await?;
            if !stopped {
                match target {
                    Some(t) => tracing::error!(
                        "No matching command found for '{}'. Use `vrw list` to see running commands.", t
                    ),
                    None => tracing::error!(
                        "No command to stop. Use `vrw list` to see running commands."
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
        Some(Commands::Resize { target, rows, cols, interactive }) => {
            subcommands::handle_resize_command(cli, target.as_deref(), *rows, *cols, *interactive).await?;
            Ok(true)
        }
        Some(Commands::Cat {
            target,
            plain,
            interactive,
        }) => {
            subcommands::handle_cat_command_http(cli, target.as_deref(), *plain, *interactive).await?;
            Ok(true)
        }
        Some(Commands::Screenshot {
            target,
            output,
            font_size,
            font_name,
            interactive,
        }) => {
            subcommands::handle_screenshot_command(
                cli,
                target.as_deref(),
                output.as_deref(),
                *font_size,
                font_name.as_deref(),
                *interactive,
            )
            .await?;
            Ok(true)
        }
        Some(Commands::Cert { .. }) | Some(Commands::ConfigCheck) | Some(Commands::Completions { .. }) => unreachable!(),
        None => Ok(false),
    }
}
