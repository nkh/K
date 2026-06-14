#![cfg(feature = "vrw")]

use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;
use serde_json::Value;

use crate::web::response::{api_err, api_ok};
use crate::web::state::AppState;

/// Request body for attaching a new output handle to a command.
///
/// ```json
/// { "name": "stdout", "sink": "file", "path": "/tmp/output.log" }
/// ```
#[derive(Debug, Deserialize)]
pub struct AddHandleRequest {
    /// Logical name for the handle (must be unique per command).
    pub name: String,
    /// Sink type: "file", "vtty", or "null".
    pub sink: String,
    /// File path for "file" sinks.  Supports `{id}` and `{name}` placeholders.
    /// Ignored for "vtty" and "null" sinks.
    pub path: Option<String>,
}

pub async fn list_handles(State(state): State<AppState>, Path(id): Path<String>) -> Json<Value> {
    match state.manager.get(&id) {
        Some(handle) => {
            let handles = handle.list_handles();
            api_ok(serde_json::json!({ "id": id, "handles": handles }))
        }
        None => api_err(format!("Command {} not found", id)),
    }
}

/// Attach a new output handle to a running command.
///
/// Creates the requested sink (file / vtty / null) and registers it in the
/// command's [`HandleRegistry`](crate::handles::registry::HandleRegistry) so
/// that output is directed to it.
///
/// # Request
///
/// ```json
/// POST /api/commands/:id/handles
/// { "name": "stdout", "sink": "file", "path": "/tmp/output-{id}.log" }
/// ```
pub async fn add_handle(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    // Parse request body
    let req = match serde_json::from_value::<AddHandleRequest>(body) {
        Ok(r) => r,
        Err(e) => {
            return api_err(format!("Invalid request: {}", e));
        }
    };

    state.manager.logger().log(
        "add_handle",
        &format!(
            "id={} name={} sink={} path={:?}",
            id, req.name, req.sink, req.path
        ),
    );

    match state
        .manager
        .register_sink(&id, req.name.clone(), &req.sink, req.path.as_deref())
    {
        Ok(()) => api_ok(serde_json::json!({
            "id": id,
            "name": req.name,
            "sink": req.sink,
            "message": "Handle attached successfully"
        })),
        Err(e) => api_err(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_handle_request_missing_required_fields() {
        let json = r#"{"name":"test"}"#;
        let result = serde_json::from_str::<AddHandleRequest>(json);
        assert!(result.is_err());
    }
}