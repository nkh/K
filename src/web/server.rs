use std::sync::Arc;
use anyhow::Result;
use tokio::sync::broadcast;

use crate::process::manager::CommandManager;
use crate::web::certs::CertificateStore;
use crate::config::schema::Config;
use super::router::create_router;
use super::state::AppState;
use super::tls::TlsManager;

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
            tracing::warn!("Failed to initialize certificate store: {}, continuing without certs", e);
            Arc::new(CertificateStore::new())
        }
    };

    let state = AppState::new(manager, shutdown_tx.clone(), auth_token, cert_store);
    let router = create_router(state);
    let app = router.into_make_service();

    let addr_str = format!("{}:{}", bind, port);
    let handle = axum_server::Handle::new();

    // Spawn graceful shutdown watcher
    let mut shutdown_rx = shutdown_tx.subscribe();
    let shutdown_handle = handle.clone();
    tokio::spawn(async move {
        let _ = shutdown_rx.recv().await;
        tracing::info!("Graceful shutdown initiated");
        shutdown_handle.graceful_shutdown(None);
    });

    // Spawn signal handler (sends on shutdown channel)
    spawn_signal_handler(shutdown_tx);

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

fn spawn_signal_handler(shutdown_tx: broadcast::Sender<()>) {
    tokio::spawn(async move {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};
            let mut sigint = signal(SignalKind::interrupt()).unwrap();
            let mut sigterm = signal(SignalKind::terminate()).unwrap();
            tokio::select! {
                _ = sigint.recv() => {},
                _ = sigterm.recv() => {},
            }
        }
        #[cfg(not(unix))]
        {
            let _ = tokio::signal::ctrl_c().await;
        }
        let _ = shutdown_tx.send(());
    });
}
