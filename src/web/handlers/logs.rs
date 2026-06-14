#![cfg(feature = "vrw")]

use axum::{
    extract::{Query, State},
    Json,
};
use serde_json::Value;
use std::collections::HashMap;

use crate::web::response::api_ok;
use crate::web::state::AppState;

/// Read command log contents with optional search/filter.
///
/// When a log file is configured, reads from the file.  Otherwise falls back
/// to the in-memory ring buffer kept by `CommandLogger` so that `--log`
/// (without `--log-file`) still produces entries visible in the web UI.
pub async fn get_log(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Json<Value> {
    let search = params.get("search").map(|s| s.as_str()).unwrap_or("");
    let limit = params
        .get("limit")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(200);
    let offset = params
        .get("offset")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(0);

    let cfg = &state.manager.config().command_log;

    // If logging is not enabled at all, return early with a clear message
    if !cfg.enabled {
        return api_ok(serde_json::json!({
            "lines": [],
            "total_lines": 0,
            "filtered_lines": 0,
            "offset": 0,
            "limit": limit,
            "search": search,
            "message": "Command logging is not enabled. Start vrw with --log or --log-file <path> to enable.",
        }));
    }

    // Try to read from the configured log file path first
    let file_lines: Option<Vec<String>> = cfg.file.as_ref().and_then(|path| {
        let content = std::fs::read_to_string(path).ok()?;
        if content.is_empty() {
            return None;
        }
        Some(content.lines().map(|l| l.to_string()).collect())
    });

    match file_lines {
        Some(lines) => {
            let total = lines.len();

            // Filter by search term if provided
            let filtered: Vec<String> = if !search.is_empty() {
                lines
                    .iter()
                    .filter(|line| line.to_lowercase().contains(&search.to_lowercase()))
                    .cloned()
                    .collect()
            } else {
                lines
            };

            let filtered_total = filtered.len();
            let page: Vec<String> = filtered.into_iter().skip(offset).take(limit).collect();

            api_ok(serde_json::json!({
                "lines": page,
                "total_lines": total,
                "filtered_lines": filtered_total,
                "offset": offset,
                "limit": limit,
                "search": search,
            }))
        }
        None => {
            // No log file configured — fall back to the in-memory ring buffer
            let mem_lines = state.manager.logger().read_memory_buffer();

            if mem_lines.is_empty() {
                return api_ok(serde_json::json!({
                    "lines": [],
                    "total_lines": 0,
                    "filtered_lines": 0,
                    "offset": 0,
                    "limit": limit,
                    "search": search,
                    "message": "No log entries yet. Logs will appear here once commands are spawned (spawn, kill, resize, etc.).",
                }));
            }

            let total = mem_lines.len();

            let filtered: Vec<String> = if !search.is_empty() {
                mem_lines
                    .iter()
                    .filter(|line| line.to_lowercase().contains(&search.to_lowercase()))
                    .cloned()
                    .collect()
            } else {
                mem_lines
            };

            let filtered_total = filtered.len();
            let page: Vec<String> = filtered.into_iter().skip(offset).take(limit).collect();

            api_ok(serde_json::json!({
                "lines": page,
                "total_lines": total,
                "filtered_lines": filtered_total,
                "offset": offset,
                "limit": limit,
                "search": search,
                "source": "memory",
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::schema::Config;
    use crate::process::manager::CommandManager;
    use crate::web::certs::CertificateStore;
    use crate::web::state::AppState;
    use std::sync::Arc;

    fn make_app_state() -> AppState {
        let mut config = Config::default();
        config.binary_name = "test".to_string();
        let manager = Arc::new(CommandManager::new(config));
        let (shutdown_tx, _) = tokio::sync::broadcast::channel::<()>(1);
        let cert_store = Arc::new(CertificateStore::new());
        let (vtty_tx, _) = tokio::sync::broadcast::channel::<(String, String)>(16);
        let (log_tx, _) = tokio::sync::broadcast::channel::<String>(16);
        AppState::new(manager, shutdown_tx, None, cert_store, vtty_tx, log_tx)
    }

    #[tokio::test]
    async fn test_get_log_disabled() {
        let state = make_app_state();
        let params: HashMap<String, String> = HashMap::new();
        let result = get_log(State(state), Query(params)).await;
        assert_eq!(result.0["status"], "ok");
        assert_eq!(result.0["data"]["total_lines"], 0);
        assert_eq!(result.0["data"]["lines"].as_array().unwrap().len(), 0);
        // Should have a message about logging not being enabled
        assert!(result.0["data"]["message"].as_str().unwrap().contains("not enabled"));
    }

    #[tokio::test]
    async fn test_get_log_with_search_param() {
        let state = make_app_state();
        let mut params: HashMap<String, String> = HashMap::new();
        params.insert("search".to_string(), "spawn".to_string());
        let result = get_log(State(state), Query(params)).await;
        assert_eq!(result.0["status"], "ok");
        assert_eq!(result.0["data"]["search"], "spawn");
        // Logging is disabled, so still empty
        assert_eq!(result.0["data"]["lines"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_get_log_with_limit_and_offset() {
        let state = make_app_state();
        let mut params: HashMap<String, String> = HashMap::new();
        params.insert("limit".to_string(), "10".to_string());
        params.insert("offset".to_string(), "5".to_string());
        let result = get_log(State(state), Query(params)).await;
        assert_eq!(result.0["status"], "ok");
        // When logging is disabled, the early-return path uses limit from the request
        assert_eq!(result.0["data"]["limit"], 10);
        // But offset is hardcoded to 0 in the disabled path
        assert_eq!(result.0["data"]["offset"], 0);
    }
}