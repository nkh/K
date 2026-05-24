use axum::{
    extract::{Query, State},
    Json,
};
use serde_json::Value;
use std::collections::HashMap;

use crate::web::state::AppState;

/// Read command log contents with optional search/filter.
/// Returns log entries from the configured log file.  When logging is enabled
/// but no file path is set, logs go to stdout only and are not available via
/// this endpoint — the response includes a helpful message.
pub async fn get_log(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Json<Value> {
    let search = params.get("search").map(|s| s.as_str()).unwrap_or("");
    let limit = params.get("limit").and_then(|v| v.parse::<usize>().ok()).unwrap_or(200);
    let offset = params.get("offset").and_then(|v| v.parse::<usize>().ok()).unwrap_or(0);

    let cfg = &state.manager.config().command_log;

    // If logging is not enabled at all, return early with a clear message
    if !cfg.enabled {
        return Json(serde_json::json!({
            "status": "ok",
            "data": {
                "lines": [],
                "total_lines": 0,
                "filtered_lines": 0,
                "offset": 0,
                "limit": limit,
                "search": search,
                "message": "Command logging is not enabled. Start vrunner with --log or --log-file <path> to enable.",
            },
            "error": null
        }));
    }

    // Try to read from the configured log file path
    let log_content = cfg.file.as_ref().and_then(|path| std::fs::read_to_string(path).ok()).filter(|c| !c.is_empty());

    match log_content {
        Some(content) => {
            let lines: Vec<&str> = content.lines().collect();
            let total = lines.len();

            // Filter by search term if provided
            let filtered: Vec<&str> = if !search.is_empty() {
                lines.iter()
                    .filter(|line| line.to_lowercase().contains(&search.to_lowercase()))
                    .copied()
                    .collect()
            } else {
                lines
            };

            let filtered_total = filtered.len();
            let page: Vec<&str> = filtered
                .into_iter()
                .skip(offset)
                .take(limit)
                .collect();

            Json(serde_json::json!({
                "status": "ok",
                "data": {
                    "lines": page,
                    "total_lines": total,
                    "filtered_lines": filtered_total,
                    "offset": offset,
                    "limit": limit,
                    "search": search,
                },
                "error": null
            }))
        }
        None => Json(serde_json::json!({
            "status": "ok",
            "data": {
                "lines": [],
                "total_lines": 0,
                "filtered_lines": 0,
                "offset": 0,
                "limit": limit,
                "search": search,
                "message": "Logging is enabled but no log file is configured. Use --log-file <path> or set command_log.file in config.",
            },
            "error": null
        })),
    }
}
