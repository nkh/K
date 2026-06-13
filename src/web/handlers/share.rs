#![cfg(feature = "vrw")]
#![allow(dead_code, unused_imports)]
use axum::{
    extract::{Path, State},
    Json,
};
use serde_json::Value;

use crate::web::state::AppState;
use std::time::Duration;

/// POST /api/commands/:id/share
/// Create a share token for a command's terminal output.
///
/// Body: { "keyboard": false, "expires_hours": 24 }
/// Returns: { "status": "ok", "data": { "token": "...", "url": "...", "expires_at": "..." } }
pub async fn create_share_token(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let keyboard = body
        .get("keyboard")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let expires_hours = body
        .get("expires_hours")
        .and_then(|v| v.as_u64())
        .unwrap_or(24);

    let expires_at = if expires_hours > 0 {
        Some(std::time::Instant::now() + Duration::from_secs(expires_hours * 3600))
    } else {
        None
    };

    // Verify the command exists
    let exists = state.manager.get(&id).is_some();
    if !exists {
        return Json(serde_json::json!({
            "status": "error",
            "data": null,
            "error": format!("Command '{}' not found", id)
        }));
    }

    let token = uuid::Uuid::new_v4().to_string();
    let share = crate::web::state::ShareToken {
        cmd_id: id.clone(),
        keyboard,
        expires_at,
    };
    state.share_tokens.insert(token.clone(), share);

    let expires_at_str = expires_at
        .map(|t| {
            let secs = t.duration_since(std::time::Instant::now()).as_secs();
            format!("{}h from now", secs / 3600)
        })
        .unwrap_or_else(|| "never".to_string());

    Json(serde_json::json!({
        "status": "ok",
        "data": {
            "token": token,
            "url": format!("/share/{}", token),
            "expires_at": expires_at_str,
            "keyboard": keyboard,
        },
        "error": null
    }))
}

/// GET /api/share/:token
/// Validate a share token and return the command's VTTY HTML.
pub async fn get_share(State(state): State<AppState>, Path(token): Path<String>) -> Json<Value> {
    let Some(entry) = state.share_tokens.get(&token) else {
        return Json(serde_json::json!({
            "status": "error",
            "data": null,
            "error": "Invalid or expired share token"
        }));
    };

    let share = entry.value().clone();

    // Check expiration
    if let Some(expires) = share.expires_at {
        if std::time::Instant::now() >= expires {
            drop(entry);
            state.share_tokens.remove(&token);
            return Json(serde_json::json!({
                "status": "error",
                "data": null,
                "error": "Share token has expired"
            }));
        }
    }

    let cmd_id = share.cmd_id;
    drop(entry);

    // Fetch the VTTY HTML for the command
    let html = state
        .manager
        .get(&cmd_id)
        .map(|h| async move { h.vtty_html().await })
        .map(|f| tokio::task::block_in_place(|| tokio::runtime::Handle::current().block_on(f)));

    let html_str = html.unwrap_or_default();

    Json(serde_json::json!({
        "status": "ok",
        "data": {
            "cmd_id": cmd_id,
            "html": html_str,
            "keyboard": share.keyboard,
        },
        "error": null
    }))
}


