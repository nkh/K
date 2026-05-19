use std::sync::Arc;
use axum::{
    extract::{Path, State},
    Json,
};
use serde_json::Value;

use crate::process::manager::CommandManager;

pub async fn list_handles(
    State(manager): State<Arc<CommandManager>>,
    Path(id): Path<String>,
) -> Json<Value> {
    match manager.get(&id) {
        Some(handle) => {
            let handles = handle.list_handles();
            Json(serde_json::json!({
                "status": "ok",
                "data": { "id": id, "handles": handles },
                "error": null
            }))
        }
        None => Json(serde_json::json!({
            "status": "error",
            "data": null,
            "error": format!("Command {} not found", id)
        })),
    }
}

pub async fn add_handle(
    State(manager): State<Arc<CommandManager>>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    manager.logger().log("add_handle", &format!("id={} body={}", id, body));
    match manager.get(&id) {
        Some(_handle) => {
            // TODO: Dynamic handle attachment requires additional plumbing
            Json(serde_json::json!({
                "status": "ok",
                "data": { "id": id, "message": "Handle attachment not yet fully implemented" },
                "error": null
            }))
        }
        None => Json(serde_json::json!({
            "status": "error",
            "data": null,
            "error": format!("Command {} not found", id)
        })),
    }
}
