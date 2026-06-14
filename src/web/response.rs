//! Standardized JSON API response helpers.
//!
//! All handlers return `axum::Json<serde_json::Value>` using the envelope format:
//! ```json
//! { "status": "ok"|"error", "data": ..., "error": ... }
//! ```
//!
//! Use [`api_ok`] and [`api_err`] to avoid hand-building the envelope in every handler.

use axum::Json;
use serde_json::{json, Value};

/// Wrap data in the standard API success envelope: `{ "status": "ok", "data": ..., "error": null }`.
#[inline]
pub fn api_ok<T: serde::Serialize>(data: T) -> Json<Value> {
    Json(json!({
        "status": "ok",
        "data": data,
        "error": null
    }))
}

/// Build a standard API error envelope: `{ "status": "error", "data": null, "error": "..." }`.
#[inline]
pub fn api_err(msg: impl std::fmt::Display) -> Json<Value> {
    Json(json!({
        "status": "error",
        "data": null,
        "error": msg.to_string()
    }))
}