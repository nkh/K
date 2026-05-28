use std::sync::Arc;
use std::time::Instant;
use tokio::sync::broadcast;

use crate::process::manager::CommandManager;
use crate::web::certs::CertificateStore;
use dashmap::DashMap;

/// A share token that grants read-only (or interactive) access to a command's terminal.
#[derive(Clone)]
pub struct ShareToken {
    /// The command UUID this token grants access to.
    pub cmd_id: String,
    /// Whether keyboard interaction is enabled for this share link.
    pub keyboard: bool,
    /// When this token expires (None = never).
    pub expires_at: Option<Instant>,
}

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
    /// Broadcast sender for VTTY change notifications. `(command_id, html_content)`.
    pub vtty_events: broadcast::Sender<(String, String)>,
    /// Broadcast sender for log entries.
    pub log_events: broadcast::Sender<String>,
    /// In-memory store of share tokens keyed by token string.
    pub share_tokens: Arc<DashMap<String, ShareToken>>,
}

impl AppState {
    pub fn new(
        manager: Arc<CommandManager>,
        shutdown_tx: broadcast::Sender<()>,
        auth_token: Option<String>,
        cert_store: Arc<CertificateStore>,
        vtty_events: broadcast::Sender<(String, String)>,
        log_events: broadcast::Sender<String>,
    ) -> Self {
        Self {
            manager,
            shutdown_tx,
            auth_token,
            cert_store,
            vtty_events,
            log_events,
            share_tokens: Arc::new(DashMap::new()),
        }
    }

    /// Remove expired share tokens. Call periodically from a background task.
    pub fn cleanup_expired_share_tokens(&self) {
        let now = Instant::now();
        self.share_tokens.retain(|_, v| match v.expires_at {
            Some(exp) => exp > now,
            None => true,
        });
    }
}
