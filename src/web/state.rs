use std::sync::Arc;
use tokio::sync::broadcast;

use crate::process::manager::CommandManager;
use crate::web::certs::CertificateStore;

/// Shared application state, threaded through Axum's state extractor.
/// All dependencies are explicit — no globals, no unsafe, no OnceLock hacks.
#[derive(Clone)]
pub struct AppState {
    pub manager: Arc<CommandManager>,
    pub shutdown_tx: broadcast::Sender<()>,
    /// The bearer token for API authentication. `None` means auth is disabled.
    pub auth_token: Option<String>,
    /// The certificate store for per-command access control.
    pub cert_store: Arc<CertificateStore>,
}

impl AppState {
    pub fn new(
        manager: Arc<CommandManager>,
        shutdown_tx: broadcast::Sender<()>,
        auth_token: Option<String>,
        cert_store: Arc<CertificateStore>,
    ) -> Self {
        Self { manager, shutdown_tx, auth_token, cert_store }
    }
}
