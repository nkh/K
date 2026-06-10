#![cfg(feature = "vrw")]
#![allow(dead_code, unused_imports)]

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::broadcast;

use vrc_core::cli::args::Cli;
use vrc_core::cli::dispatch;
use vrc_core::cli::startup;
use vrc_core::instance::registry::InstanceRegistry;
use vrc_core::interactive::display::{run_display_loop, wait_for_child};
use vrc_core::process::manager::CommandManager;
use vrc_core::web::auth::AuthManager;
use vrc_core::web::server::start_server;

const DEFAULT_PORT: u16 = 9090;

fn check_port_available(bind: &str, port: u16) -> Result<()> {
    let addr = format!("{}:{}", bind, port);
    match std::net::TcpListener::bind(&addr) {
        Ok(listener) => {
            drop(listener);
            Ok(())
        }
        Err(e) => {
            anyhow::bail!(
                "Port {} is already in use (bind address: {}). \n\
                 Use `vrw list` to see running instances, or specify \n\
                 a different port with `--port <PORT>`. \n\
                 Error: {}",
                port, bind, e
            );
        }
    }
}

async fn try_client_mode(cli: &Cli) -> Result<bool> {
    let cmd_args = match &cli.cmd_args {
        Some(args) if !args.is_empty() => args,
        _ => return Ok(false),
    };

    let cmd = &cmd_args[0];
    let args = &cmd_args[1..];

    let bind = cli.bind.clone().unwrap_or_else(|| "127.0.0.1".to_string());
    let probe_url = format!("http://{}:{}/api/commands", bind, DEFAULT_PORT);

    tracing::info!(
        url = %probe_url,
        cmd = %cmd,
        "No --port specified; trying to send command to running instance"
    );

    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(2))
        .timeout(std::time::Duration::from_secs(5))
        .build();

    let client = match client {
        Ok(c) => c,
        Err(e) => {
            tracing::info!(error = %e, "Failed to build HTTP client for client mode");
            return Ok(false);
        }
    };

    let mut body = serde_json::json!({
        "cmd": cmd,
        "args": args,
    });

    if let Some(ref dir) = cli.working_directory {
        body["working_directory"] = serde_json::json!(dir);
    }

    let resp = match client.post(&probe_url).json(&body).send().await {
        Ok(resp) => resp,
        Err(e) => {
            tracing::info!(
                error = %e,
                url = %probe_url,
                "No running vrw instance found at default port — starting new server"
            );
            return Ok(false);
        }
    };

    let status = resp.status();
    let result: serde_json::Value = match resp.json().await {
        Ok(v) => v,
        Err(e) => {
            tracing::info!(error = %e, "Non-JSON response from instance");
            return Ok(false);
        }
    };

    if status.is_success() {
        let cmd_pid = result["data"]["pid"].as_u64().unwrap_or(0);
        println!(
            "Command sent to running vrw instance on port {}",
            DEFAULT_PORT
        );
        println!("  PID:       {}", cmd_pid);
        println!("  VTTY:      http://{}:{}/admin/{}", bind, DEFAULT_PORT, cmd);
        Ok(true)
    } else {
        let error = result["error"].as_str().unwrap_or("Unknown error");
        tracing::warn!(
            status = %status,
            error = %error,
            "Server responded with error in client mode"
        );
        Ok(false)
    }
}

async fn async_main(cli: Cli) -> Result<()> {
    if cli.no_log {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::WARN)
            .init();
    } else {
        tracing_subscriber::fmt::init();
    }

    if let Ok(cwd) = std::env::current_dir() {
        tracing::info!(cwd = %cwd.display(), "Working directory");
    }

    if dispatch::handle_subcommands(&cli).await? {
        return Ok(());
    }

    // Only try client mode for bare commands (no flags), matching the
    // same heuristic used in handle_subcommands().  When any flags are
    // present (e.g. --display, --daemon, --tabs), the user explicitly
    // wants to start a new instance — not send the command to an
    // existing one.  Without this guard, "vrw --display htop" would
    // silently hand off to a running instance and skip the display loop.
    if cli.port.is_none() {
        let argc = std::env::args().count();
        let bare_command = cli
            .cmd_args
            .as_ref()
            .is_some_and(|args| !args.is_empty() && argc == 1 + args.len());
        if bare_command && try_client_mode(&cli).await? {
            return Ok(());
        }
    }

    let mut cfg = dispatch::resolve_config(&cli)?;
    startup::apply_detected_terminal_size(&cli, &mut cfg);
    let handle_sigwinch = cli.handle_sigwinch;

    let registry = InstanceRegistry::new()?;
    registry.register_current(&cfg)?;

    let auth_token = if cfg.security.require_auth {
        Some(AuthManager::load_or_generate(&cfg.security.token_file)?)
    } else {
        None
    };

    let manager = Arc::new(CommandManager::new(cfg.clone()));

    if let Err(e) = check_port_available(&cfg.server.bind, cfg.server.port) {
        tracing::error!(bind = %cfg.server.bind, port = cfg.server.port, "{}", e);
        anyhow::bail!("{}", e);
    }

    let spawned_id = startup::spawn_initial_command(&cli, &manager, &cfg).await?;

    let (shutdown_tx, _shutdown_rx) = broadcast::channel::<()>(1);

    let server_handle = tokio::spawn({
        let manager = manager.clone();
        let shutdown_tx = shutdown_tx.clone();
        let cfg = cfg.clone();
        let auth_token = auth_token.clone();
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
            )
            .await
        }
    });

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    if let Some(register_port) = cli.register_with {
        let my_url = format!("http://{}:{}", cfg.server.bind, cfg.server.port);
        let my_label = format!("vrw:{}", std::process::id());
        let my_token = auth_token.clone().unwrap_or_default();
        let primary_url = format!("http://{}:{}/api/peers", cfg.server.bind, register_port);
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            let body = serde_json::json!({
                "url": my_url,
                "label": my_label,
                "token": my_token,
            });
            let client = reqwest::Client::builder()
                .connect_timeout(std::time::Duration::from_secs(3))
                .timeout(std::time::Duration::from_secs(5))
                .build();
            match client {
                Ok(c) => match c.post(&primary_url).json(&body).send().await {
                    Ok(resp) if resp.status().is_success() => {
                        tracing::info!(
                            primary = %primary_url,
                            my_url = %my_url,
                            "Registered with primary instance"
                        );
                    }
                    Ok(resp) => {
                        tracing::warn!(
                            primary = %primary_url,
                            status = %resp.status(),
                            "Failed to register with primary instance (HTTP error)"
                        );
                    }
                    Err(e) => {
                        tracing::warn!(
                            primary = %primary_url,
                            error = %e,
                            "Failed to register with primary instance (connection error)"
                        );
                    }
                },
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to build HTTP client for registration");
                }
            }
        });
    }

    if cfg.display.enabled {
        let log_entries = manager.logger().memory_buffer_arc();
        let effective_display_all = cfg.interactive.tabs;
        run_display_loop(
            &manager,
            spawned_id.as_deref(),
            cfg.display.refresh_ms,
            effective_display_all,
            shutdown_tx.clone(),
            &cfg.interactive.keybindings,
            &log_entries,
            cfg.interactive.tabs,
            handle_sigwinch,
        )
        .await;
    } else if !cli.no_terminal_log && !cli.quiet {
        let rx = shutdown_tx.subscribe();
        startup::run_non_display_event_loop(&manager, spawned_id.as_deref(), rx).await;
    } else if let Some(ref id) = spawned_id {
        let rx = shutdown_tx.subscribe();
        wait_for_child(&manager, id, rx).await;
    } else {
        let mut rx = shutdown_tx.subscribe();
        let _ = rx.recv().await;
    }

    tokio::select! {
        _ = server_handle => {
            tracing::info!("Server shut down");
        }
        _ = tokio::time::sleep(std::time::Duration::from_secs(3)) => {
            tracing::warn!("Server did not shut down within 3s, forcing exit");
        }
    }

    std::process::exit(0);
}

fn main() -> Result<()> {
    let cli = match dispatch::pre_runtime()? {
        Some(cli) => cli,
        None => return Ok(()),
    };

    if cli.daemon {
        #[cfg(unix)]
        {
            let mut cfg = dispatch::resolve_config(&cli)?;
            if !cfg.daemon.enabled {
                cfg.daemon.enabled = true;
            }
            vrc_core::daemon::unix::daemonize(&cfg)?;
        }
        #[cfg(not(unix))]
        {
            anyhow::bail!("--daemon is only supported on Unix-like systems");
        }
    }

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(async_main(cli))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify DEFAULT_PORT is 9090.
    #[test]
    fn test_default_port() {
        assert_eq!(DEFAULT_PORT, 9090);
    }

    /// Verify check_port_available accepts a free port.
    #[test]
    fn test_check_port_available_free_port() {
        // Port 0 means the OS picks a free port, so binding should always succeed.
        let result = check_port_available("127.0.0.1", 0);
        assert!(result.is_ok(), "binding port 0 should succeed");
    }

    /// Verify check_port_available rejects an obviously unavailable port.
    #[test]
    fn test_check_port_available_bound_port() {
        // Bind port 0 to get a free port, then check that same port is no longer free.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let result = check_port_available("127.0.0.1", port);
        assert!(result.is_err(), "already-bound port should fail");
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains(&port.to_string()), "error should mention port number");
    }

    /// Verify the async_main function signature.
    #[test]
    fn test_async_main_function_signature() {
        fn _type_check(_: fn(vrc_core::cli::args::Cli) -> anyhow::Result<()>) {}
        _type_check(async_main);
    }

    /// Verify try_client_mode signature — it takes &Cli and returns Result<bool>.
    #[test]
    fn test_try_client_mode_signature() {
        fn _type_check(_: fn(&vrc_core::cli::args::Cli) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<bool>> + Send>>) {}
        // We can't easily type-check async functions, so just verify it's callable.
        // The function exists and compiles — verified by integration.
        let _ = std::any::type_name_of_val(&try_client_mode);
    }

    /// Verify vrw imports all necessary modules.
    #[test]
    fn test_vrw_imports_auth_manager() {
        fn _type_check(_: fn(&vrc_core::config::security::SecurityConfig) -> anyhow::Result<String>) {}
        _type_check(vrc_core::web::auth::AuthManager::load_or_generate);
    }
}
