#![cfg(feature = "vrw")]
#![allow(dead_code, unused_imports)]
use axum::extract::State;
use axum::Json;
use serde_json::Value;

use crate::web::state::AppState;

/// GET /api/templates
///
/// Returns the list of command templates defined in the server configuration.
/// These are the `[[templates]]` entries from the config file.
pub async fn list_templates(State(state): State<AppState>) -> Json<Value> {
    let config = state.manager.config();
    let templates = &config.templates;

    let data: Vec<Value> = templates
        .iter()
        .map(|t| {
            serde_json::json!({
                "name": t.name,
                "cmd": t.cmd,
                "args": t.args,
                "env": t.env,
                "workdir": t.workdir,
                "certificate": t.certificate,
                "rows": t.rows,
                "cols": t.cols,
            })
        })
        .collect();

    Json(serde_json::json!({
        "status": "ok",
        "data": data,
        "error": null
    }))
}
