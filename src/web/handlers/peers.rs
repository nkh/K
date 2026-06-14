#![cfg(feature = "vrw")]

//! Peer instance registration and discovery.

use axum::{extract::{Path, State}, Json};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::web::response::{api_err, api_ok};
use crate::web::state::AppState;

/// Information about a registered peer vrw instance.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PeerInfo {
    /// The base URL of the peer's web server (e.g. "http://localhost:9091").
    pub url: String,
    /// Human-readable label for the peer.
    pub label: String,
    /// Auth token for the peer's API (empty if auth is disabled on the peer).
    pub token: String,
    /// PID of the peer vrw process.
    pub pid: u32,
    /// When this peer was registered.
    #[serde(with = "chrono::serde::ts_seconds_option")]
    pub registered_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Request body for peer registration.
#[derive(Deserialize)]
pub struct RegisterPeerRequest {
    pub url: String,
    pub label: Option<String>,
    #[serde(default)]
    pub token: String,
}

/// List all registered peer instances.
pub async fn list_peers(State(state): State<AppState>) -> Json<Value> {
    let peers: Vec<PeerInfo> = state.peers.iter().map(|r| r.value().clone()).collect();
    api_ok(serde_json::json!(peers))
}

/// Register a new peer instance.
///
/// Called by a vrw server (or manually via curl) to announce itself
/// to this server. The peer info is stored in memory and broadcast to
/// all connected WebSocket clients.
pub async fn register_peer(
    State(state): State<AppState>,
    Json(body): Json<RegisterPeerRequest>,
) -> Json<Value> {
    // Validate URL is well-formed
    if body.url.is_empty() {
        return api_err("url is required");
    }

    let label = body
        .label
        .unwrap_or_else(|| format!("vrw:{}", std::process::id()));

    let peer = PeerInfo {
        url: body.url.clone(),
        label,
        token: body.token,
        pid: std::process::id(),
        registered_at: Some(chrono::Utc::now()),
    };

    let is_new = !state.peers.contains_key(&body.url);
    state.peers.insert(body.url.clone(), peer.clone());

    tracing::info!(
        url = %body.url,
        label = %peer.label,
        is_new,
        "Peer registered"
    );

    // Broadcast to connected WebSocket clients
    if is_new {
        let msg = serde_json::json!({
            "type": "peer_registered",
            "data": {
                "url": peer.url,
                "label": peer.label,
                "token": peer.token,
            }
        })
        .to_string();
        let _ = state.peer_events.send(msg);
    }

    api_ok(serde_json::json!(peer))
}

/// Unregister a peer instance.
///
/// Called when a peer is shutting down or wants to remove itself.
pub async fn unregister_peer(
    State(state): State<AppState>,
    Path(url): Path<String>,
) -> Json<Value> {
    if state.peers.remove(&url).is_some() {
        tracing::info!(url = %url, "Peer unregistered");

        let msg = serde_json::json!({
            "type": "peer_unregistered",
            "data": { "url": url }
        })
        .to_string();
        let _ = state.peer_events.send(msg);
    }

    api_ok(Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::schema::Config;
    use crate::process::manager::CommandManager;
    use crate::web::certs::CertificateStore;
    use crate::web::state::AppState;
    use std::sync::Arc;

    fn make_app_state() -> AppState {
        let mut config = Config::default();
        config.binary_name = "test".to_string();
        let manager = Arc::new(CommandManager::new(config));
        let (shutdown_tx, _) = tokio::sync::broadcast::channel::<()>(1);
        let cert_store = Arc::new(CertificateStore::new());
        let (vtty_tx, _) = tokio::sync::broadcast::channel::<(String, String)>(16);
        let (log_tx, _) = tokio::sync::broadcast::channel::<String>(16);
        AppState::new(manager, shutdown_tx, None, cert_store, vtty_tx, log_tx)
    }

    #[tokio::test]
    async fn test_list_peers_empty() {
        let state = make_app_state();
        let result = list_peers(State(state)).await;
        assert_eq!(result.0["status"], "ok");
        assert_eq!(result.0["data"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_register_peer_success() {
        let state = make_app_state();
        let body = RegisterPeerRequest {
            url: "http://localhost:9091".to_string(),
            label: Some("peer-1".to_string()),
            token: "secret".to_string(),
        };
        let result = register_peer(State(state.clone()), Json(body)).await;
        assert_eq!(result.0["status"], "ok");
        assert_eq!(result.0["data"]["url"], "http://localhost:9091");
        assert_eq!(result.0["data"]["label"], "peer-1");
        assert_eq!(result.0["data"]["token"], "secret");
        assert!(result.0["data"]["registered_at"].is_number());
        // Verify it's stored
        assert!(state.peers.contains_key("http://localhost:9091"));
    }

    #[tokio::test]
    async fn test_register_peer_empty_url() {
        let state = make_app_state();
        let body = RegisterPeerRequest {
            url: "".to_string(),
            label: None,
            token: "".to_string(),
        };
        let result = register_peer(State(state), Json(body)).await;
        assert_eq!(result.0["status"], "error");
        assert!(result.0["error"].as_str().unwrap().contains("url is required"));
    }

    #[tokio::test]
    async fn test_register_peer_default_label() {
        let state = make_app_state();
        let body = RegisterPeerRequest {
            url: "http://localhost:9092".to_string(),
            label: None,
            token: "".to_string(),
        };
        let result = register_peer(State(state), Json(body)).await;
        assert_eq!(result.0["status"], "ok");
        // Default label should be "vrw:<pid>"
        assert!(result.0["data"]["label"].as_str().unwrap().starts_with("vrw:"));
    }

    #[tokio::test]
    async fn test_register_peer_idempotent() {
        let state = make_app_state();
        let body1 = RegisterPeerRequest {
            url: "http://localhost:9093".to_string(),
            label: Some("peer-x".to_string()),
            token: "tok".to_string(),
        };
        let body2 = RegisterPeerRequest {
            url: "http://localhost:9093".to_string(),
            label: Some("peer-x".to_string()),
            token: "tok".to_string(),
        };
        // Register twice with same URL
        let _ = register_peer(State(state.clone()), Json(body1)).await;
        let result = register_peer(State(state.clone()), Json(body2)).await;
        assert_eq!(result.0["status"], "ok");
        // Should still be exactly one peer
        assert_eq!(state.peers.len(), 1);
    }

    #[tokio::test]
    async fn test_unregister_peer_existing() {
        let state = make_app_state();
        // Pre-insert a peer
        state.peers.insert("http://localhost:9094".to_string(), PeerInfo {
            url: "http://localhost:9094".to_string(),
            label: "to-remove".to_string(),
            token: "".to_string(),
            pid: 12345,
            registered_at: None,
        });
        let result = unregister_peer(State(state.clone()), Path("http://localhost:9094".to_string())).await;
        assert_eq!(result.0["status"], "ok");
        assert!(!state.peers.contains_key("http://localhost:9094"));
    }

    #[tokio::test]
    async fn test_unregister_peer_nonexistent() {
        let state = make_app_state();
        let result = unregister_peer(State(state), Path("http://nonexistent:9999".to_string())).await;
        assert_eq!(result.0["status"], "ok");
        // Should succeed even if peer doesn't exist (idempotent)
        assert_eq!(result.0["data"], Value::Null);
    }

    #[tokio::test]
    async fn test_list_peers_after_register() {
        let state = make_app_state();
        let body = RegisterPeerRequest {
            url: "http://localhost:9095".to_string(),
            label: Some("visible-peer".to_string()),
            token: "".to_string(),
        };
        register_peer(State(state.clone()), Json(body)).await;
        let result = list_peers(State(state)).await;
        assert_eq!(result.0["status"], "ok");
        let data = result.0["data"].as_array().unwrap();
        assert_eq!(data.len(), 1);
        assert_eq!(data[0]["url"], "http://localhost:9095");
        assert_eq!(data[0]["label"], "visible-peer");
    }
}