use anyhow::Result;
use clap::Parser;
use std::sync::Arc;
use tokio::sync::broadcast;

use vrunner::cli::args::{Cli, Commands, CertAction};
use vrunner::config::loader::load_config;
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

    // Load and merge configuration
    let mut cfg = load_config(cli.config.as_deref())?;
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
    if let Some(cmd_args) = cli.cmd_args {
        if !cmd_args.is_empty() {
            let cmd = cmd_args[0].clone();
            let args = cmd_args[1..].to_vec();
            let _id = manager.spawn(cmd, args, None).await?;
        }
    }

    // Create shutdown channel — passed explicitly, no globals
    let (shutdown_tx, _shutdown_rx) = broadcast::channel::<()>(1);

    // Start the web server
    let server_handle = tokio::spawn(async move {
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
    });

    // Wait for server to finish
    let _ = server_handle.await?;

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
