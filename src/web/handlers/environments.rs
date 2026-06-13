#![cfg(feature = "vrw")]
#![allow(dead_code, unused_imports)]
use axum::extract::State;
use axum::Json;
use serde_json::Value;

use crate::web::state::AppState;

/// GET /api/environments
///
/// Returns the list of workspace environments defined in the server
/// configuration.  These are the `[[environments]]` entries from the
/// config file.
pub async fn list_environments(State(state): State<AppState>) -> Json<Value> {
    let config = state.manager.config();
    let environments = &config.environments;

    let data: Vec<Value> = environments
        .iter()
        .map(|e| {
            let panels: Vec<Value> = e
                .panels
                .iter()
                .map(|p| {
                    let cmds: Vec<Value> = p
                        .commands
                        .iter()
                        .map(|c| {
                            serde_json::json!({
                                "cmd": c.cmd,
                                "args": c.args,
                                "workdir": c.workdir,
                                "certificate": c.certificate,
                                "rows": c.rows,
                                "cols": c.cols,
                                "retain_on_exit": c.retain_on_exit,
                            })
                        })
                        .collect();
                    serde_json::json!({
                        "title": p.title,
                        "server": p.server,
                        "token": p.token,
                        "server_label": p.server_label,
                        "commands": cmds,
                    })
                })
                .collect();
            serde_json::json!({
                "name": e.name,
                "description": e.description,
                "layout": e.layout,
                "auto_start": e.auto_start,
                "default_server": e.default_server,
                "panels": panels,
            })
        })
        .collect();

    Json(serde_json::json!({
        "status": "ok",
        "data": data,
        "error": null
    }))
}


