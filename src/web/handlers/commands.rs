use axum::{
    extract::{Path, State},
    Json,
};
use serde_json::Value;

use crate::web::state::AppState;

pub async fn list_commands(
    State(state): State<AppState>,
) -> Json<Value> {
    let commands = state.manager.list();
    let data: Vec<Value> = commands.into_iter()
        .map(|(id, name, pid, certificate)| {
            serde_json::json!({
                "id": id,
                "name": name,
                "pid": pid,
                "status": "running",
                "certificate": certificate,
            })
        })
        .collect();
    Json(serde_json::json!({ "status": "ok", "data": data, "error": null }))
}

pub async fn start_command(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let cmd = body.get("cmd").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let args: Vec<String> = body.get("args")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();
    let certificate: Option<String> = body.get("certificate")
        .and_then(|v| v.as_str())
        .map(String::from);

    if cmd.is_empty() {
        return Json(serde_json::json!({
            "status": "error",
            "data": null,
            "error": "Missing 'cmd' field"
        }));
    }

    // Validate certificate name if provided
    if let Some(ref cert_name) = certificate {
        if !state.cert_store.exists(cert_name) {
            return Json(serde_json::json!({
                "status": "error",
                "data": null,
                "error": format!("Certificate '{}' not found in store", cert_name)
            }));
        }
    }

    match state.manager.spawn(cmd, args, certificate).await {
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
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let signal = body.get("signal").and_then(|v| v.as_str()).map(String::from);
    match state.manager.kill(&id, signal).await {
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
    State(state): State<AppState>,
) -> Json<Value> {
    let _ = state.shutdown_tx.send(());
    Json(serde_json::json!({
        "status": "ok",
        "data": { "message": "shutdown initiated" },
        "error": null
    }))
}
