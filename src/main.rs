use anyhow::Result;
use clap::Parser;
use std::sync::Arc;
use tokio::sync::broadcast;

use vrunner::cli::args::{Cli, Commands, CertAction};
use vrunner::config::loader::load_config;
use vrunner::config::merge::apply_profile;
use vrunner::daemon;
use vrunner::instance::registry::InstanceRegistry;
use vrunner::process::manager::CommandManager;
use vrunner::web::auth::AuthManager;
use vrunner::web::certs::CertificateStore;
use vrunner::web::server::start_server;

/// Synchronous pre-runtime phase: parse CLI, handle subcommands, load config,
/// and daemonize. Daemonization MUST happen before the tokio runtime starts,
/// because fork() only copies the calling thread while tokio's multi-threaded
/// runtime creates internal threads for I/O, timers, and blocking tasks.
fn pre_runtime() -> Result<Option<Cli>> {
    let cli = Cli::parse();

    // Handle subcommands that don't need the runtime
    match &cli.command {
        Some(Commands::List) => {
            let registry = InstanceRegistry::new()?;
            registry.print_list();
            return Ok(None); // Exit without starting runtime
        }
        Some(Commands::Stop { pid: _ }) => {
            // stop_instance is async (uses reqwest), so we need the runtime
            // Fall through to the async phase
        }
        Some(Commands::Spawn { .. }) => {
            // spawn is async (uses reqwest), fall through to async phase
        }
        Some(Commands::Freeze { .. }) => {
            // freeze is async (uses reqwest), fall through to async phase
        }
        Some(Commands::Thaw { .. }) => {
            // thaw is async (uses reqwest), fall through to async phase
        }
        Some(Commands::Cert { action }) => {
            // Cert subcommands are synchronous — handle them here
            handle_cert_command(action)?;
            return Ok(None);
        }
        None => {}
    }

    Ok(Some(cli))
}

/// Async runtime phase: start the server and manage the application lifecycle.
async fn async_main(cli: Cli) -> Result<()> {
    // Initialize tracing (after daemonize, so logs go to the right place)
    tracing_subscriber::fmt::init();

    // Handle stop subcommand (needs async for HTTP request)
    if let Some(Commands::Stop { pid }) = cli.command {
        let registry = InstanceRegistry::new()?;
        registry.stop_instance(pid).await?;
        return Ok(());
    }

    // Handle spawn subcommand — send to a running vrunner instance
    if let Some(Commands::Spawn { ref cmd, ref args }) = cli.command {
        handle_spawn_command(&cli, &cmd, &args).await?;
        return Ok(());
    }

    // Handle freeze subcommand
    if let Some(Commands::Freeze { ref id }) = cli.command {
        handle_freeze_command(&cli, &id).await?;
        return Ok(());
    }

    // Handle thaw subcommand
    if let Some(Commands::Thaw { ref id }) = cli.command {
        handle_thaw_command(&cli, &id).await?;
        return Ok(());
    }

    // Load and merge configuration
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
                    cfg.profiles.entries.keys().cloned().collect::<Vec<_>>().join(", ")
                }
            );
        }
    }

    // Apply CLI overrides (highest precedence)
    cli.apply_overrides(&mut cfg);

    // Initialize instance registry
    let registry = InstanceRegistry::new()?;
    registry.register_current(&cfg)?;

    // Load or generate auth token if auth is required
    let auth_token = if cfg.security.require_auth {
        Some(AuthManager::load_or_generate(&cfg.security.token_file)?)
    } else {
        None
    };

    // Initialize command manager
    let manager = Arc::new(CommandManager::new(cfg.clone()));

    // If a child command was provided, spawn it immediately
    let spawned_id = if let Some(cmd_args) = cli.cmd_args {
        if !cmd_args.is_empty() {
            let cmd = cmd_args[0].clone();
            let args = cmd_args[1..].to_vec();
            let id = manager.spawn(cmd, args, None, cfg.environment.variables.clone()).await?;
            Some(id)
        } else {
            None
        }
    } else {
        None
    };

    // Create shutdown channel — passed explicitly, no globals
    let (shutdown_tx, _shutdown_rx) = broadcast::channel::<()>(1);

    // Start the web server
    let server_handle = tokio::spawn({
        let manager = manager.clone();
        let shutdown_tx = shutdown_tx.clone();
        let cfg = cfg.clone();
        async move {
            start_server(
                cfg.server.bind.clone(),
                cfg.server.port,
                manager.clone(),
                shutdown_tx,
                auth_token,
                cfg.tls.enabled,
                cfg.tls.cert_file.as_deref(),
                cfg.tls.key_file.as_deref(),
                &cfg,
            ).await
        }
    });

    // If --display is enabled, run a local terminal display loop that renders
    // the active command's VTTY output directly to stdout (like mprocs).
    if cfg.display.enabled {
        let display_manager = manager.clone();
        let display_id = spawned_id.clone();
        let mut shutdown_rx = shutdown_tx.subscribe();
        let refresh_ms = cfg.display.refresh_ms;

        tokio::spawn(async move {
            use vrunner::vtty::display::TerminalDisplay;
            use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
            use crossterm::{cursor, ExecutableCommand};

            // Switch to alternate screen for our display so we don't
            // corrupt the user's terminal history.
            let mut stdout = std::io::stdout();
            let _ = terminal::enable_raw_mode();
            let _ = stdout.execute(EnterAlternateScreen);
            let _ = stdout.execute(cursor::Hide);

            let mut last_html: String = String::new();
            let interval = tokio::time::Duration::from_millis(refresh_ms);

            loop {
                tokio::select! {
                    _ = tokio::time::sleep(interval) => {
                        // Find a command to display
                        let commands = display_manager.list();
                        let target_id = display_id.as_ref()
                            .or_else(|| commands.first().map(|(id, _, _, _)| id));

                        if let Some(id) = target_id {
                            if let Some(handle) = display_manager.get(id) {
                                let html = handle.vtty_html().await;
                                drop(handle);
                                if html != last_html {
                                    last_html = html;
                                    // Render the buffer directly to the terminal
                                    let buf = display_manager.get(id).unwrap().vtty_snapshot().await;
                                    let _ = TerminalDisplay::render(&buf);
                                }
                            } else {
                                // Command was removed
                                let _ = TerminalDisplay::clear();
                                break;
                            }
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        break;
                    }
                }
            }

            // Restore terminal
            let _ = stdout.execute(cursor::Show);
            let _ = stdout.execute(LeaveAlternateScreen);
            let _ = terminal::disable_raw_mode();
        });
    }

    // Wait for server to finish — propagate both JoinError and server errors
    server_handle.await??;

    // Cleanup on exit
    registry.unregister_current()?;

    Ok(())
}

fn main() -> Result<()> {
    // Phase 1: Synchronous pre-runtime (no tokio threads yet)
    let cli = match pre_runtime()? {
        Some(cli) => cli,
        None => return Ok(()), // Subcommand handled, exit
    };

    // Daemonize if requested — MUST happen before tokio::runtime is created.
    // At this point, only the main thread exists, so fork() is safe.
    // After daemonization, the original process exits and the daemon
    // (grandchild of fork) continues as the new process.
    if cli.daemon {
        #[cfg(unix)]
        {
            // For daemon mode, we need to load config early to get log file paths.
            let cfg = load_config(cli.config.as_deref())?;
            let mut cfg = cfg;

            // Apply profile if specified
            if let Some(ref profile_name) = cli.profile {
                if let Some(profile) = cfg.profiles.entries.clone().get(profile_name) {
                    cfg = apply_profile(cfg, profile);
                }
            }

            cli.apply_overrides(&mut cfg);

            if !cfg.daemon.enabled {
                // CLI --daemon flag overrides config
                cfg.daemon.enabled = true;
            }

            daemon::unix::daemonize(&cfg)?;
            // After daemonize(), we are the daemon process.
            // Only the main thread exists — safe to start tokio now.
        }
        #[cfg(not(unix))]
        {
            anyhow::bail!("--daemon is only supported on Unix-like systems");
        }
    }

    // Phase 2: Start tokio runtime and run async main
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(async_main(cli))
}

/// Build the base URL for a vrunner instance, handling auth and TLS.
fn instance_url(info: &vrunner::instance::info::InstanceInfo, _auth_token: &Option<String>) -> String {
    let scheme = if info.port == 443 { "https" } else { "http" };
    let mut url = format!("{}://{}:{}", scheme, info.bind, info.port);
    // For simplicity, we try HTTP first. TLS instances will reject and
    // the error message will guide the user.
    url = format!("http://{}:{}", info.bind, info.port);
    url
}

/// Discover running vrunner instances and resolve to a single target.
/// Returns the selected InstanceInfo or an error.
fn resolve_instance(
    cli: &Cli,
    registry: &InstanceRegistry,
) -> Result<vrunner::instance::info::InstanceInfo> {
    let instances = registry.list_instances();

    if instances.is_empty() {
        anyhow::bail!("No running vrunner instances found. Start one first with: vrunner -- <command>");
    }

    // If --target PID was specified, use that instance
    if let Some(target_pid) = cli.target {
        match instances.iter().find(|i| i.pid == target_pid) {
            Some(info) => return Ok(info.clone()),
            None => anyhow::bail!(
                "No vrunner instance found with PID {}. Running instances:\n{}",
                target_pid,
                format_instance_list(&instances)
            ),
        }
    }

    // Only one instance — use it automatically
    if instances.len() == 1 {
        return Ok(instances.into_iter().next().unwrap());
    }

    // Multiple instances — prompt the user
    eprintln!("Multiple vrunner instances are running:");
    eprintln!("{}", format_instance_list(&instances));
    eprintln!();
    eprint!("Enter the PID of the instance to use (or Ctrl+C to abort): ");
    eprintln!();

    // Since we can't easily read stdin in all contexts (piped, daemon, etc.),
    // return an error with instructions
    anyhow::bail!(
        "Multiple vrunner instances are running. Use --target PID to select one.\n\
         Running instances:\n{}",
        format_instance_list(&instances)
    );
}

fn format_instance_list(instances: &[vrunner::instance::info::InstanceInfo]) -> String {
    let mut out = String::new();
    out.push_str(&format!("{:<10} {:<8} {:<20} {:<10} {:<10} COMMAND\n",
        "PID", "PORT", "BIND", "DAEMON", "DISPLAY"));
    for info in instances {
        out.push_str(&format!("{:<10} {:<8} {:<20} {:<10} {:<10} {}\n",
            info.pid,
            info.port,
            info.bind,
            if info.daemon { "yes" } else { "no" },
            if info.display { "yes" } else { "no" },
            info.command.as_deref().unwrap_or("(idle)")
        ));
    }
    out
}

/// Handle the `vrunner spawn` subcommand.
/// Discovers a running vrunner instance and sends a spawn request via HTTP API.
async fn handle_spawn_command(cli: &Cli, cmd: &str, args: &[String]) -> Result<()> {
    let registry = InstanceRegistry::new()?;
    let info = resolve_instance(cli, &registry)?;

    let url = instance_url(&info, &None);
    let client = reqwest::Client::new();

    let mut body = serde_json::json!({
        "cmd": cmd,
        "args": args,
    });

    // Add --env variables if provided
    let cli_env = cli.parse_env_vars();
    if !cli_env.is_empty() {
        body["env"] = serde_json::json!(cli_env);
    }

    // Add --no-env flag to skip config-level environment
    if cli.no_env {
        body["no_env"] = serde_json::json!(true);
    }

    // Add exit configuration if provided
    if let Some(ref on_exit) = cli.on_exit {
        body["on_exit"] = serde_json::json!(on_exit);
    }
    if let Some(ref on_error) = cli.on_error {
        body["on_error"] = serde_json::json!(on_error);
    }
    if let Some(timeout) = cli.exit_timeout {
        body["exit_timeout"] = serde_json::json!(timeout);
    }

    // Add profile if specified
    if let Some(ref profile) = cli.profile {
        body["profile"] = serde_json::json!(profile);
    }

    tracing::info!(target_pid = info.pid, cmd = cmd, "Spawning command on remote instance");

    let resp = client
        .post(format!("{}/api/commands", url))
        .json(&body)
        .send()
        .await?;

    let status = resp.status();
    let result: serde_json::Value = resp.json().await?;

    if status.is_success() {
        let id = result["data"]["id"].as_str().unwrap_or("?");
        println!("Command spawned successfully on instance {} (PID {})", info.pid, info.pid);
        println!("  Command ID: {}", id);
        println!("  VTTY:      {}/api/commands/{}/vtty/html", url, id);
    } else {
        let error = result["error"].as_str().unwrap_or("Unknown error");
        eprintln!("Failed to spawn command: {}", error);
        std::process::exit(1);
    }

    Ok(())
}

/// Handle the `vrunner freeze` subcommand.
async fn handle_freeze_command(cli: &Cli, id: &str) -> Result<()> {
    let registry = InstanceRegistry::new()?;
    let info = resolve_instance(cli, &registry)?;
    let url = instance_url(&info, &None);
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{}/api/commands/{}/freeze", url, id))
        .send()
        .await?;

    let status = resp.status();
    let result: serde_json::Value = resp.json().await?;

    if status.is_success() {
        println!("Command {} frozen (SIGSTOP) on instance {}", id, info.pid);
    } else {
        let error = result["error"].as_str().unwrap_or("Unknown error");
        eprintln!("Failed to freeze command: {}", error);
        std::process::exit(1);
    }

    Ok(())
}

/// Handle the `vrunner thaw` subcommand.
async fn handle_thaw_command(cli: &Cli, id: &str) -> Result<()> {
    let registry = InstanceRegistry::new()?;
    let info = resolve_instance(cli, &registry)?;
    let url = instance_url(&info, &None);
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{}/api/commands/{}/thaw", url, id))
        .send()
        .await?;

    let status = resp.status();
    let result: serde_json::Value = resp.json().await?;

    if status.is_success() {
        println!("Command {} thawed (SIGCONT) on instance {}", id, info.pid);
    } else {
        let error = result["error"].as_str().unwrap_or("Unknown error");
        eprintln!("Failed to thaw command: {}", error);
        std::process::exit(1);
    }

    Ok(())
}

/// Handle `vrunner cert` subcommands (generate, list, show, remove).
///
/// These are synchronous operations that don't require the tokio runtime.
fn handle_cert_command(action: &CertAction) -> Result<()> {
    match action {
        CertAction::Generate { name } => {
            let mut store = CertificateStore::new();
            let entry = store.generate(name)?;
            let token = entry.derive_token()?;
            println!("Certificate '{}' generated successfully.", name);
            println!("  Certificate: {}", entry.cert_file);
            println!("  Key:        {}", entry.key_file);
            println!("  Token:      {}... (first 16 of 64 chars)", &token[..16]);
        }
        CertAction::List => {
            let cfg = load_config(None)?;
            let entries: Vec<vrunner::web::certs::CertificateEntry> = cfg
                .certificates
                .entries
                .iter()
                .map(|e| vrunner::web::certs::CertificateEntry {
                    name: e.name.clone(),
                    cert_file: e.cert_file.clone(),
                    key_file: e.key_file.clone(),
                })
                .collect();

            if entries.is_empty() {
                println!("No certificates configured.");
                return Ok(());
            }

            match CertificateStore::load_or_generate(entries) {
                Ok(store) => {
                    let certs = store.list();
                    if certs.is_empty() {
                        println!("No certificates in the store.");
                    } else {
                        println!("{:<25} {:<50} {}", "NAME", "CERT FILE", "TOKEN (prefix)");
                        println!("{}", "-".repeat(100));
                        for cert in certs {
                            let token_preview = cert
                                .derive_token()
                                .map(|t| format!("{}...", &t[..16]))
                                .unwrap_or_else(|_| "<error>".to_string());
                            println!("{:<25} {:<50} {}", cert.name, cert.cert_file, token_preview);
                        }
                    }
                }
                Err(e) => {
                    anyhow::bail!("Failed to load certificates: {}", e);
                }
            }
        }
        CertAction::Show { name } => {
            let cfg = load_config(None)?;
            let entries: Vec<vrunner::web::certs::CertificateEntry> = cfg
                .certificates
                .entries
                .iter()
                .map(|e| vrunner::web::certs::CertificateEntry {
                    name: e.name.clone(),
                    cert_file: e.cert_file.clone(),
                    key_file: e.key_file.clone(),
                })
                .collect();

            let store = CertificateStore::load_or_generate(entries)?;

            match store.get(name) {
                Some(entry) => {
                    let token = entry.derive_token()?;
                    println!("Certificate: {}", entry.name);
                    println!("  Certificate: {}", entry.cert_file);
                    println!("  Key:        {}", entry.key_file);
                    println!("  Token:      {} (full SHA-256 hex)", token);
                    println!("  Token (16): {}...", &token[..16]);
                }
                None => {
                    anyhow::bail!("Certificate '{}' not found in store", name);
                }
            }
        }
        CertAction::Remove { name } => {
            let cfg = load_config(None)?;
            let entries: Vec<vrunner::web::certs::CertificateEntry> = cfg
                .certificates
                .entries
                .iter()
                .map(|e| vrunner::web::certs::CertificateEntry {
                    name: e.name.clone(),
                    cert_file: e.cert_file.clone(),
                    key_file: e.key_file.clone(),
                })
                .collect();

            let mut store = CertificateStore::load_or_generate(entries)?;

            match store.remove(name) {
                Some(entry) => {
                    println!("Certificate '{}' removed from store.", name);
                    println!("  Certificate: {}", entry.cert_file);
                    println!("  Key:        {}", entry.key_file);
                    println!("  Note: Files were not deleted.");
                }
                None => {
                    anyhow::bail!("Certificate '{}' not found in store", name);
                }
            }
        }
    }
    Ok(())
}
