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
        Some(_handle) => {
            // TODO: Return actual handle list
            Json(serde_json::json!({
                "status": "ok",
                "data": { "id": id, "handles": [] },
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
            // TODO: Attach new handle to running command
            Json(serde_json::json!({
                "status": "ok",
                "data": { "id": id },
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
