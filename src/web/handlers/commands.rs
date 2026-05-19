use std::sync::Arc;
use axum::{
    extract::{Path, State},
    Json,
};
use serde_json::Value;

use crate::process::manager::CommandManager;
use crate::main::get_shutdown_tx;

pub async fn list_commands(
    State(manager): State<Arc<CommandManager>>,
) -> Json<Value> {
    let commands = manager.list();
    let data: Vec<Value> = commands.into_iter()
        .map(|(id, name, pid)| {
            serde_json::json!({
                "id": id,
                "name": name,
                "pid": pid,
                "status": "running"
            })
        })
        .collect();
    Json(serde_json::json!({ "status": "ok", "data": data, "error": null }))
}

pub async fn start_command(
    State(manager): State<Arc<CommandManager>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let cmd = body.get("cmd").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let args: Vec<String> = body.get("args")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();

    if cmd.is_empty() {
        return Json(serde_json::json!({
            "status": "error",
            "data": null,
            "error": "Missing 'cmd' field"
        }));
    }

    match manager.spawn(cmd, args).await {
        Ok(id) => Json(serde_json::json!({
            "status": "ok",
            "data": { "id": id },
            "error": null
        })),
        Err(e) => Json(serde_json::json!({
            "status": "error",
            "data": null,
            "error": e.to_string()
        })),
    }
}

pub async fn kill_command(
    State(manager): State<Arc<CommandManager>>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let signal = body.get("signal").and_then(|v| v.as_str()).map(String::from);
    match manager.kill(&id, signal).await {
        Ok(_) => Json(serde_json::json!({
            "status": "ok",
            "data": { "id": id },
            "error": null
        })),
        Err(e) => Json(serde_json::json!({
            "status": "error",
            "data": null,
            "error": e.to_string()
        })),
    }
}

pub async fn shutdown(
    State(_manager): State<Arc<CommandManager>>,
) -> Json<Value> {
    if let Some(tx) = get_shutdown_tx() {
        let _ = tx.send(());
        Json(serde_json::json!({
            "status": "ok",
            "data": { "message": "shutdown initiated" },
            "error": null
        }))
    } else {
        Json(serde_json::json!({
            "status": "error",
            "data": null,
            "error": "Shutdown channel not initialized"
        }))
    }
}
