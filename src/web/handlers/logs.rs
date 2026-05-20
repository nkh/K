use axum::{
    extract::{Query, State},
    Json,
};
use serde_json::Value;
use std::collections::HashMap;

use crate::web::state::AppState;

/// Read command log contents with optional search/filter.
pub async fn get_log(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Json<Value> {
    let search = params.get("search").map(|s| s.as_str()).unwrap_or("");
    let limit = params.get("limit").and_then(|v| v.parse::<usize>().ok()).unwrap_or(200);
    let offset = params.get("offset").and_then(|v| v.parse::<usize>().ok()).unwrap_or(0);

    // Try to read log from the command logger
    // The logger is accessed via the manager, but we need the file path.
    // For now, check common log locations.
    let log_content = read_log_contents(&state);

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
                "message": "No log file configured or file not found",
            },
            "error": null
        })),
    }
}

fn read_log_contents(_state: &AppState) -> Option<String> {
    // Try common log file paths since we don't store the path in AppState directly
    let paths = [
        "/tmp/vrunner.log",
        "./vrunner.log",
        "./vrunner-commands.log",
    ];

    for path in &paths {
        if let Ok(content) = std::fs::read_to_string(path) {
            if !content.is_empty() {
                return Some(content);
            }
        }
    }

    // Also check the daemon stdout/stderr files
    let daemon_paths = [
        "/tmp/vrunner.out",
        "/tmp/vrunner.err",
    ];

    for path in &daemon_paths {
        if let Ok(content) = std::fs::read_to_string(path) {
            if !content.is_empty() {
                return Some(content);
            }
        }
    }

    None
}
