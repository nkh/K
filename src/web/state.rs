#![cfg(feature = "vrw")]

use std::sync::Arc;
use std::time::Instant;
use tokio::sync::broadcast;

use crate::process::manager::CommandManager;
use crate::web::certs::CertificateStore;
use crate::web::handlers::peers::PeerInfo;
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
    /// Registered peer vrw instances (url -> peer info).
    pub peers: Arc<DashMap<String, PeerInfo>>,
    /// Broadcast sender for peer registration/unregistration events.
    /// Messages are pre-serialized JSON strings forwarded to WS clients.
    pub peer_events: broadcast::Sender<String>,
    /// Max dirty signals per session per burst window.
    pub max_burst: u32,
    /// Burst window duration in milliseconds.
    pub burst_window_ms: u32,
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
        let (peer_events_tx, _) = broadcast::channel::<String>(16);
        Self {
            manager,
            shutdown_tx,
            auth_token,
            cert_store,
            vtty_events,
            log_events,
            share_tokens: Arc::new(DashMap::new()),
            peers: Arc::new(DashMap::new()),
            peer_events: peer_events_tx,
            max_burst: 10,
            burst_window_ms: 1000,
        }
    }

    /// Create AppState with explicit burst throttle settings.
    pub fn with_throttle(
        manager: Arc<CommandManager>,
        shutdown_tx: broadcast::Sender<()>,
        auth_token: Option<String>,
        cert_store: Arc<CertificateStore>,
        vtty_events: broadcast::Sender<(String, String)>,
        log_events: broadcast::Sender<String>,
        max_burst: u32,
        burst_window_ms: u32,
    ) -> Self {
        let (peer_events_tx, _) = broadcast::channel::<String>(16);
        Self {
            manager,
            shutdown_tx,
            auth_token,
            cert_store,
            vtty_events,
            log_events,
            share_tokens: Arc::new(DashMap::new()),
            peers: Arc::new(DashMap::new()),
            peer_events: peer_events_tx,
            max_burst,
            burst_window_ms,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web::certs::CertificateStore;
    use crate::config::schema::Config;

    fn make_test_state() -> AppState {
        let cfg = Config::default();
        let manager = Arc::new(crate::process::manager::CommandManager::new(cfg));
        let (shutdown_tx, _) = tokio::sync::broadcast::channel::<()>(1);
        let (vtty_events, _) = tokio::sync::broadcast::channel(16);
        let (log_events, _) = tokio::sync::broadcast::channel(16);
        let cert_store = Arc::new(CertificateStore::new());
        AppState::new(
            manager,
            shutdown_tx,
            None,
            cert_store,
            vtty_events,
            log_events,
        )
    }

    #[test]
    fn test_cleanup_expired_share_tokens() {
        let state = make_test_state();
        // Add a non-expiring token
        state.share_tokens.insert("never-expire".to_string(), ShareToken {
            cmd_id: "cmd1".to_string(),
            keyboard: false,
            expires_at: None,
        });
        // Add an already-expired token
        state.share_tokens.insert("expired".to_string(), ShareToken {
            cmd_id: "cmd2".to_string(),
            keyboard: false,
            expires_at: Some(std::time::Instant::now() - std::time::Duration::from_secs(1)),
        });
        assert_eq!(state.share_tokens.len(), 2);
        state.cleanup_expired_share_tokens();
        assert_eq!(state.share_tokens.len(), 1);
        assert!(state.share_tokens.contains_key("never-expire"));
    }
}
