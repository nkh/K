#![cfg(feature = "vrw")]
#![allow(dead_code, unused_imports)]
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::broadcast;

use super::router::create_router;
use super::state::AppState;
use super::tls::TlsManager;
use crate::config::schema::Config;
use crate::process::manager::CommandManager;
use crate::web::certs::CertificateStore;

#[allow(clippy::too_many_arguments)]
pub async fn start_server(
    bind: String,
    port: u16,
    manager: Arc<CommandManager>,
    shutdown_tx: broadcast::Sender<()>,
    auth_token: Option<String>,
    tls_enabled: bool,
    tls_cert_file: Option<&str>,
    tls_key_file: Option<&str>,
    config: &Config,
) -> Result<()> {
    // Initialize certificate store from config
    let cert_entries: Vec<crate::web::certs::CertificateEntry> = config
        .certificates
        .entries
        .iter()
        .map(|e| crate::web::certs::CertificateEntry {
            name: e.name.clone(),
            cert_file: e.cert_file.clone(),
            key_file: e.key_file.clone(),
        })
        .collect();

    let cert_store = match CertificateStore::load_or_generate(cert_entries) {
        Ok(store) => {
            let count = store.list().len();
            if count > 0 {
                tracing::info!("Certificate store loaded with {} certificate(s)", count);
            }
            Arc::new(store)
        }
        Err(e) => {
            tracing::warn!(
                "Failed to initialize certificate store: {}, continuing without certs",
                e
            );
            Arc::new(CertificateStore::new())
        }
    };

    let vtty_events = manager.vtty_change_sender();
    let log_events = manager.logger().log_sender();
    let state = AppState::new(
        manager,
        shutdown_tx.clone(),
        auth_token,
        cert_store,
        vtty_events,
        log_events,
    );
    let router = create_router(state, &config.security.cors);
    let app = router.into_make_service();

    let addr_str = format!("{}:{}", bind, port);
    let handle = axum_server::Handle::new();

    // Spawn graceful shutdown watcher
    let mut shutdown_rx = shutdown_tx.subscribe();
    let shutdown_handle = handle.clone();
    tokio::spawn(async move {
        let _ = shutdown_rx.recv().await;
        tracing::info!("Graceful shutdown initiated");
        // Use a 2-second timeout so vrw doesn't hang waiting for
        // persistent connections (HTTP keep-alive, SSE, WebSocket) to drain.
        shutdown_handle.graceful_shutdown(Some(std::time::Duration::from_secs(2)));
    });

    // Spawn signal handler (sends on shutdown channel)
    crate::cli::startup::spawn_signal_handler(shutdown_tx);

    if tls_enabled {
        tracing::info!("Server listening on https://{}", addr_str);

        // TLS mode — generate or load certificates
        let tls_config = TlsManager::load_or_generate_config(tls_cert_file, tls_key_file)?;
        let rustls_config = axum_server::tls_rustls::RustlsConfig::from_config(tls_config);

        let server = axum_server::bind_rustls(addr_str.parse()?, rustls_config)
            .handle(handle)
            .serve(app);

        server.await?;
    } else {
        tracing::info!("Server listening on http://{}", addr_str);

        let server = axum_server::bind(addr_str.parse()?)
            .handle(handle)
            .serve(app);

        server.await?;
    }

    Ok(())
}
