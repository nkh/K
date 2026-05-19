use std::sync::Arc;
use axum::{
    extract::{Path, State, Query},
    Json,
};
use serde_json::Value;
use std::collections::HashMap;

use crate::process::manager::CommandManager;

pub async fn get_vtty_full(
    State(manager): State<Arc<CommandManager>>,
    Path(id): Path<String>,
) -> Json<Value> {
    match manager.get(&id) {
        Some(handle) => {
            let ansi = handle.vtty_ansi().await;
            Json(serde_json::json!({
                "status": "ok",
                "data": { "id": id, "content": ansi },
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

pub async fn get_vtty_partial(
    State(manager): State<Arc<CommandManager>>,
    Path(id): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Json<Value> {
    let offset = params.get("offset").and_then(|v| v.parse::<usize>().ok()).unwrap_or(0);
    let limit = params.get("limit").and_then(|v| v.parse::<usize>().ok()).unwrap_or(50);

    match manager.get(&id) {
        Some(handle) => {
            let content = handle.vtty_partial(offset, limit).await;
            Json(serde_json::json!({
                "status": "ok",
                "data": {
                    "id": id,
                    "offset": offset,
                    "limit": limit,
                    "content": content
                },
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
