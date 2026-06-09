#![cfg(feature = "vrw")]
#![allow(dead_code, unused_imports)]
//! Peer instance registration and discovery.
//!
//! Allows vrw instances to register with each other so the web UI
//! can display commands from multiple servers. When the primary server
//! exits, the browser can fail over to a registered peer.
//!
//! Registration flow:
//! 1. Server B starts with `--register-with 9090`
//! 2. After binding, Server B POSTs to `http://localhost:9090/api/peers`
//! 3. Server A stores the peer info and broadcasts to connected WS clients
//! 4. The JS client adds the peer to its instance list
//! 5. When Server A exits, the JS tries peers in order

use axum::{extract::{Path, State}, Json};
use serde::{Deserialize, Serialize};

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
pub async fn list_peers(State(state): State<AppState>) -> Json<serde_json::Value> {
    let peers: Vec<PeerInfo> = state.peers.iter().map(|r| r.value().clone()).collect();
    Json(serde_json::json!({
        "status": "ok",
        "data": peers,
    }))
}

/// Register a new peer instance.
///
/// Called by a vrw server (or manually via curl) to announce itself
/// to this server. The peer info is stored in memory and broadcast to
/// all connected WebSocket clients.
pub async fn register_peer(
    State(state): State<AppState>,
    Json(body): Json<RegisterPeerRequest>,
) -> Json<serde_json::Value> {
    // Validate URL is well-formed
    if body.url.is_empty() {
        return Json(serde_json::json!({
            "status": "error",
            "error": "url is required"
        }));
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

    Json(serde_json::json!({
        "status": "ok",
        "data": peer,
    }))
}

/// Unregister a peer instance.
///
/// Called when a peer is shutting down or wants to remove itself.
pub async fn unregister_peer(
    State(state): State<AppState>,
    Path(url): Path<String>,
) -> Json<serde_json::Value> {
    if state.peers.remove(&url).is_some() {
        tracing::info!(url = %url, "Peer unregistered");

        let msg = serde_json::json!({
            "type": "peer_unregistered",
            "data": { "url": url }
        })
        .to_string();
        let _ = state.peer_events.send(msg);
    }

    Json(serde_json::json!({"status": "ok"}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_peer_info_serialization() {
        let peer = PeerInfo {
            url: "http://localhost:9091".to_string(),
            label: "vrw:12345".to_string(),
            token: "secret".to_string(),
            pid: 12345,
            registered_at: Some(chrono::Utc::now()),
        };
        let json = serde_json::to_string(&peer).unwrap();
        assert!(json.contains("localhost:9091"));
        assert!(json.contains("vrw:12345"));
    }

    #[test]
    fn test_peer_info_deserialization() {
        let json = serde_json::json!({
            "url": "http://localhost:9092",
            "label": "Peer2",
            "token": "tok123",
            "pid": 9999,
            "registered_at": 1700000000
        });
        let peer: PeerInfo = serde_json::from_value(json).unwrap();
        assert_eq!(peer.url, "http://localhost:9092");
        assert_eq!(peer.label, "Peer2");
        assert_eq!(peer.token, "tok123");
        assert_eq!(peer.pid, 9999);
        assert!(peer.registered_at.is_some());
    }

    #[test]
    fn test_peer_info_clone() {
        let peer = PeerInfo {
            url: "http://localhost:9091".to_string(),
            label: "test".to_string(),
            token: "".to_string(),
            pid: 1,
            registered_at: None,
        };
        let cloned = peer.clone();
        assert_eq!(cloned.url, peer.url);
    }

    #[test]
    fn test_peer_info_debug() {
        let peer = PeerInfo {
            url: "http://localhost:9091".to_string(),
            label: "test".to_string(),
            token: "".to_string(),
            pid: 1,
            registered_at: None,
        };
        let debug_str = format!("{:?}", peer);
        assert!(debug_str.contains("PeerInfo"));
    }

    #[test]
    fn test_register_peer_request_deserialization() {
        let json = serde_json::json!({
            "url": "http://localhost:9091",
            "label": "TestPeer"
        });
        let req: RegisterPeerRequest = serde_json::from_value(json).unwrap();
        assert_eq!(req.url, "http://localhost:9091");
        assert_eq!(req.label, Some("TestPeer".to_string()));
        assert!(req.token.is_empty());
    }

    #[test]
    fn test_register_peer_request_without_label() {
        let json = serde_json::json!({
            "url": "http://localhost:9091"
        });
        let req: RegisterPeerRequest = serde_json::from_value(json).unwrap();
        assert!(req.label.is_none());
    }
}
