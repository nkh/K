#![cfg(feature = "vrw")]
#![allow(dead_code, unused_imports)]
use axum::{extract::State, Json};
use serde_json::Value;

use crate::web::state::AppState;

/// List all certificates in the store.
///
/// Returns each certificate's name, cert_file, and derived token (first 16 chars).
pub async fn list_certificates(State(state): State<AppState>) -> Json<Value> {
    let entries = state.cert_store.list();
    let data: Vec<Value> = entries
        .into_iter()
        .map(|entry| {
            let token_preview = entry
                .derive_token()
                .map(|t| t[..16].to_string())
                .unwrap_or_default();
            serde_json::json!({
                "name": entry.name,
                "cert_file": entry.cert_file,
                "key_file": entry.key_file,
                "token_preview": token_preview,
            })
        })
        .collect();
    Json(serde_json::json!({ "status": "ok", "data": data, "error": null }))
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
}
