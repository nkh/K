use std::net::SocketAddr;
use std::sync::Arc;
use anyhow::Result;
use axum::serve;
use tokio::net::TcpListener;
use tokio::sync::broadcast;

use crate::process::manager::CommandManager;
use super::router::create_router;
use super::state::AppState;

pub async fn start_server(
    bind: String,
    port: u16,
    manager: Arc<CommandManager>,
    shutdown_tx: broadcast::Sender<()>,
) -> Result<()> {
    let addr: SocketAddr = format!("{}:{}", bind, port).parse()?;
    let listener = TcpListener::bind(&addr).await?;
    let state = AppState::new(manager, shutdown_tx.clone());
    let router = create_router(state);

    let mut shutdown_rx = shutdown_tx.subscribe();

    let server = serve(listener, router)
        .with_graceful_shutdown(async move {
            let _ = shutdown_rx.recv().await;
        });

    tracing::info!("Server listening on http://{}", addr);

    // Spawn signal handler
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
            use tokio::signal;
            let _ = signal::ctrl_c().await;
        }
        let _ = shutdown_tx.send(());
    });

    server.await?;
    Ok(())
}
