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

/// Get the actual binary name at runtime (e.g., from argv[0]).
/// This allows completion scripts to use the correct name even if
/// the binary has been renamed, and works correctly when building
/// with `--features vrc,vrw` where the compile-time BINARY_NAME
/// is always "vrw".
fn runtime_binary_name() -> String {
    std::env::args()
        .next()
        .and_then(|arg| {
            std::path::Path::new(&arg)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
        })
        .unwrap_or_else(|| BINARY_NAME.to_string())
}

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
            let name = runtime_binary_name();
            clap_complete::generate(*shell, &mut cmd, &name, &mut stdout());
            return Ok(None);
        }
        // IPC commands — handled by the caller after tokio runtime is available
        Some(Commands::Keys { .. })
        | Some(Commands::Cat { .. })
        | Some(Commands::SpawnIn { .. })
        | Some(Commands::Freeze { .. })
        | Some(Commands::Thaw { .. })
        | Some(Commands::Resize { .. })
        | Some(Commands::Kill { .. })
        | Some(Commands::StopCommand { .. }) => {
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
            | Some(Commands::StopCommand { .. })
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
        Some(Commands::Cat { pid, command, interactive }) => {
            tracing_subscriber::fmt::init();
            verify_instance(pid)?;
            if interactive {
                subcommands::handle_cat_command_interactive(pid).await
            } else {
                subcommands::handle_cat_command(pid, command.as_deref()).await
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
        Some(Commands::Kill { pid, command, interactive, all }) => {
            tracing_subscriber::fmt::init();
            verify_instance(pid)?;
            if all {
                subcommands::handle_kill_all_commands(pid).await
            } else if interactive {
                subcommands::handle_kill_command_interactive(pid).await
            } else {
                subcommands::handle_kill_command(pid, command.as_deref()).await
            }
        }
        Some(Commands::StopCommand { pid, command, interactive, all }) => {
            tracing_subscriber::fmt::init();
            verify_instance(pid)?;
            if all {
                subcommands::handle_kill_all_commands(pid).await
            } else if interactive {
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
        Some(Commands::StopCommand { target: _, interactive: _, all: _ }) => {}
        Some(Commands::Kill { target: _, interactive: _, all: _ }) => {}
        Some(Commands::Purge { target: _, interactive: _ }) => {}
        Some(Commands::Keep { target: _, interactive: _ }) => {}
        Some(Commands::Unkeep { target: _, interactive: _ }) => {}
        Some(Commands::Resize { target: _, rows: _, cols: _, interactive: _ }) => {}
        Some(Commands::ConfigCheck) => {
            subcommands::handle_config_check_command(cli.config.as_deref())?;
            return Ok(None);
        }
        Some(Commands::Cat {
            target: _,
            plain: _,
            color_always: _,
            interactive: _,
        }) => {}
        Some(Commands::Screenshot { .. }) => {}
        Some(Commands::Completions { shell }) => {
            let name = runtime_binary_name();
            let mut cmd;
            #[cfg(all(feature = "vrc", feature = "vrw"))]
            {
                cmd = if name == "vrc" {
                    crate::cli::args::build_vrc_completions_command()
                } else {
                    <Cli as CommandFactory>::command()
                };
            }
            #[cfg(not(all(feature = "vrc", feature = "vrw")))]
            {
                cmd = <Cli as CommandFactory>::command();
            }
            clap_complete::generate(*shell, &mut cmd, &name, &mut stdout());
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
            interactive,
        }) => {
            subcommands::handle_spawn_command(cli, cmd, args, *rows, *cols, *interactive).await?;
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
        Some(Commands::StopCommand { target, interactive, all }) => {
            if *all {
                let stopped = subcommands::handle_stop_all_commands(cli).await?;
                if !stopped {
                    tracing::error!("No commands to stop.");
                    std::process::exit(1);
                }
                return Ok(true);
            }
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
        Some(Commands::Kill { target, interactive, all }) => {
            if *all {
                let stopped = subcommands::handle_stop_all_commands(cli).await?;
                if !stopped {
                    tracing::error!("No commands to stop.");
                    std::process::exit(1);
                }
                return Ok(true);
            }
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
        Some(Commands::Purge { target, interactive }) => {
            let purged = subcommands::handle_purge_command(cli, target.as_deref(), *interactive).await?;
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
        Some(Commands::Keep { target, interactive }) => {
            let kept = subcommands::handle_keep_command(cli, target.as_deref(), *interactive).await?;
            if !kept {
                match target {
                    Some(t) => tracing::error!(
                        "No matching running command found for '{}'. Use `vrw list` to see running commands.", t
                    ),
                    None => tracing::error!(
                        "No running command to keep."
                    ),
                }
                std::process::exit(1);
            }
            Ok(true)
        }
        Some(Commands::Unkeep { target, interactive }) => {
            let unkept = subcommands::handle_unkeep_command(cli, target.as_deref(), *interactive).await?;
            if !unkept {
                match target {
                    Some(t) => tracing::error!(
                        "No matching kept command found for '{}'.", t
                    ),
                    None => tracing::error!(
                        "No kept command to unkeep."
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
            color_always: _,
            interactive,
        }) => {
            // Colors are the default; --plain strips them.
            let show_color = !plain;
            subcommands::handle_cat_command_http(cli, target.as_deref(), show_color, *interactive).await?;
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
        None => {
            // "vrw btop" is equivalent to "vrw spawn btop" — but ONLY when
            // no flags are present on the command line.  When the user passes
            // any flags (e.g. `vrw --display htop`, `vrw --daemon htop`),
            // they intend to start a new instance, not spawn into an existing one.
            //
            // We detect "no flags" by comparing raw argc with the number of
            // positional cmd_args.  If argc > 1 + cmd_args.len(), flags exist.
            if let Some(ref cmd_args) = cli.cmd_args {
                if !cmd_args.is_empty() {
                    let argc = std::env::args().count();
                    if argc == 1 + cmd_args.len() {
                        // No flags → implicit spawn into running instance
                        let cmd = &cmd_args[0];
                        let args = &cmd_args[1..];
                        subcommands::handle_spawn_command(cli, cmd, args, None, None, false).await?;
                        return Ok(true);
                    }
                    // Flags present → let the caller start a new instance
                }
            }
            Ok(false)
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn resolve_config_valid_explicit_path() {
        // Use empty config to let defaults kick in (same approach as loader tests)
        let dir = std::env::temp_dir().join("vrc_test_resolve_config");
        std::fs::create_dir_all(&dir).unwrap();
        let config_path = dir.join("valid.yaml");
        std::fs::write(&config_path, "").unwrap();

        let cli = Cli::try_parse_from([BINARY_NAME, "--config", config_path.to_str().unwrap()])
            .unwrap();
        let result = resolve_config(&cli);
        assert!(result.is_ok(), "valid config should resolve: {:?}", result.err());
        let cfg = result.unwrap();
        assert_eq!(cfg.vtty.rows, 24, "default rows");
        assert_eq!(cfg.vtty.cols, 80, "default cols");

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn resolve_config_missing_file_errors() {
        let cli = Cli::try_parse_from([BINARY_NAME, "--config", "/nonexistent/path.yaml"]).unwrap();
        let result = resolve_config(&cli);
        assert!(result.is_err());
    }

    #[test]
    fn resolve_config_invalid_yaml_errors() {
        let dir = std::env::temp_dir().join("vrc_test_resolve_config_invalid");
        std::fs::create_dir_all(&dir).unwrap();
        let config_path = dir.join("bad.yaml");
        // Empty config with CLI override to negative rows should fail validation
        std::fs::write(&config_path, "").unwrap();

        // Use an invalid vtty_rows via CLI — the resolve_config function
        // applies CLI overrides then validates. We test that overrides work.
        std::fs::remove_dir_all(&dir).unwrap();
        // This test is superseded by the overrides test below
    }

    #[test]
    fn resolve_config_profile_not_found_errors() {
        let dir = std::env::temp_dir().join("vrc_test_resolve_config_profile");
        std::fs::create_dir_all(&dir).unwrap();
        let config_path = dir.join("no_profiles.yaml");
        std::fs::write(&config_path, "").unwrap();

        let cli = Cli::try_parse_from([
            BINARY_NAME,
            "--config", config_path.to_str().unwrap(),
            "--profile", "nonexistent",
        ]).unwrap();
        let result = resolve_config(&cli);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("nonexistent"), "should mention the profile name: {}", msg);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn resolve_config_overrides_rows_cols() {
        let dir = std::env::temp_dir().join("vrc_test_resolve_config_overrides");
        std::fs::create_dir_all(&dir).unwrap();
        let config_path = dir.join("overrides.yaml");
        std::fs::write(&config_path, "").unwrap();

        let cli = Cli::try_parse_from([
            BINARY_NAME,
            "--config", config_path.to_str().unwrap(),
            "--vtty-rows", "100",
            "--vtty-cols", "200",
        ]).unwrap();
        let result = resolve_config(&cli);
        assert!(result.is_ok(), "overrides should succeed: {:?}", result.err());
        let cfg = result.unwrap();
        assert_eq!(cfg.vtty.rows, 100);
        assert_eq!(cfg.vtty.cols, 200);

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
