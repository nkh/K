use axum::{
    extract::{Path, State},
    Json,
};
use serde_json::Value;

use crate::web::state::AppState;

pub async fn send_keys(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let keys = body.get("keys").and_then(|v| v.as_str()).unwrap_or("");

    if keys.is_empty() {
        return Json(serde_json::json!({
            "status": "error",
            "data": null,
            "error": "Missing 'keys' field"
        }));
    }

    match state.manager.send_keys(&id, keys).await {
        Ok(_) => Json(serde_json::json!({
            "status": "ok",
            "data": { "id": id, "keys_sent": keys },
            "error": null
        })),
        Err(e) => Json(serde_json::json!({
            "status": "error",
            "data": null,
            "error": e.to_string()
        })),
    }
}
