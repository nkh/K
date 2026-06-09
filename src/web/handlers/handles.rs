#![cfg(feature = "vrw")]
#![allow(dead_code, unused_imports)]
use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;
use serde_json::Value;

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
            return Json(serde_json::json!({
                "status": "error",
                "data": null,
                "error": format!("Invalid request: {}", e)
            }));
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
        Ok(()) => Json(serde_json::json!({
            "status": "ok",
            "data": {
                "id": id,
                "name": req.name,
                "sink": req.sink,
                "message": "Handle attached successfully"
            },
            "error": null
        })),
        Err(e) => Json(serde_json::json!({
            "status": "error",
            "data": null,
            "error": e.to_string()
        })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handler_module_compiles() {
        // Verify the handler module compiles successfully.
        // Handler functions require AppState which is tested separately.
        // This test ensures the module's types and imports are valid.
    }

    // ─── AddHandleRequest deserialization tests ───

    #[test]
    fn test_add_handle_request_file_sink() {
        let json = r#"{"name":"stdout","sink":"file","path":"/tmp/output.log"}"#;
        let req: AddHandleRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, "stdout");
        assert_eq!(req.sink, "file");
        assert_eq!(req.path, Some("/tmp/output.log".to_string()));
    }

    #[test]
    fn test_add_handle_request_vtty_sink() {
        let json = r#"{"name":"vtty0","sink":"vtty"}"#;
        let req: AddHandleRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, "vtty0");
        assert_eq!(req.sink, "vtty");
        assert!(req.path.is_none());
    }

    #[test]
    fn test_add_handle_request_null_sink() {
        let json = r#"{"name":"null0","sink":"null"}"#;
        let req: AddHandleRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, "null0");
        assert_eq!(req.sink, "null");
    }

    #[test]
    fn test_add_handle_request_with_placeholders() {
        let json = r#"{"name":"log","sink":"file","path":"/tmp/{id}-{name}.log"}"#;
        let req: AddHandleRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.path, Some("/tmp/{id}-{name}.log".to_string()));
    }

    #[test]
    fn test_add_handle_request_missing_required_fields() {
        let json = r#"{"name":"test"}"#;
        let result = serde_json::from_str::<AddHandleRequest>(json);
        assert!(result.is_err());
    }

    // ─── Handler function compile tests ───

    #[test]
    fn test_list_handles_function_exists() {
        let _ = std::any::type_name_of_val(&list_handles);
    }

    #[test]
    fn test_add_handle_function_exists() {
        let _ = std::any::type_name_of_val(&add_handle);
    }
}
