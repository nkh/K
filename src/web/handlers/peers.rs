//! Peer instance registration and discovery.
//!
//! Allows vrunner instances to register with each other so the web UI
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

/// Information about a registered peer vrunner instance.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PeerInfo {
    /// The base URL of the peer's web server (e.g. "http://localhost:9091").
    pub url: String,
    /// Human-readable label for the peer.
    pub label: String,
    /// Auth token for the peer's API (empty if auth is disabled on the peer).
    pub token: String,
    /// PID of the peer vrunner process.
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
/// Called by a vrunner server (or manually via curl) to announce itself
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
        .unwrap_or_else(|| format!("vrunner:{}", std::process::id()));

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
