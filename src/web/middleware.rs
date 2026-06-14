#![cfg(feature = "vrw")]
#![allow(dead_code, unused_imports)]
use axum::http::HeaderValue;
use axum::{
    extract::Request,
    http::header::AUTHORIZATION,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde_json::json;
use std::time::Instant;
use tracing;

use crate::config::security::CorsConfig;

/// CORS layer configuration.
///
/// Builds a [`tower_http::cors::CorsLayer`] from the given [`CorsConfig`].
///
/// - `"any"` — allows all origins, methods, and headers (default).
/// - `"none"` — creates a layer that does not set any `Access-Control-Allow-Origin`
///   header, effectively blocking all cross-origin requests from browsers.
/// - comma-separated list — only the specified origins are allowed; if none
///   parse successfully, falls back to permissive mode.
pub fn cors_layer(config: &CorsConfig) -> tower_http::cors::CorsLayer {
    match config.policy.as_str() {
        "none" => {
            // Block all cross-origin requests: build a layer that never sets
            // Access-Control-Allow-Origin.  We achieve this by simply not
            // calling `.allow_origin()` at all — the layer will add the CORS
            // headers but without an allow-origin entry, browsers will reject
            // the response.
            tower_http::cors::CorsLayer::new()
                .allow_methods(tower_http::cors::Any)
                .allow_headers(tower_http::cors::Any)
        }
        "any" => tower_http::cors::CorsLayer::new()
            .allow_origin(tower_http::cors::Any)
            .allow_methods(tower_http::cors::Any)
            .allow_headers(tower_http::cors::Any),
        _ => {
            // Parse comma-separated origins
            let origins: Vec<HeaderValue> = config
                .policy
                .split(',')
                .filter_map(|s| s.trim().parse::<HeaderValue>().ok())
                .collect();
            if origins.is_empty() {
                // If nothing parsed, fall back to permissive so the server
                // doesn't silently break all cross-origin access.
                tracing::warn!(
                    policy = %config.policy,
                    "security.cors.policy contains no valid origins; falling back to permissive CORS"
                );
                tower_http::cors::CorsLayer::permissive()
            } else {
                tower_http::cors::CorsLayer::new()
                    .allow_origin(origins)
                    .allow_methods(tower_http::cors::Any)
                    .allow_headers(tower_http::cors::Any)
            }
        }
    }
}

/// Authentication middleware: validates Bearer token when auth is enabled.
/// When the auth_token extension is None (localhost default), all requests are allowed.
pub async fn auth_middleware(req: Request, next: Next) -> Response {
    // Extract the auth token from extensions (set by the router layer)
    let token = req.extensions().get::<Option<String>>().cloned().flatten();

    match token {
        None => {
            // No auth required — pass through
            next.run(req).await
        }
        Some(expected) => {
            // Auth required — validate Bearer token
            let provided = req
                .headers()
                .get(AUTHORIZATION)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.strip_prefix("Bearer "));

            match provided {
                Some(t) if t == expected => next.run(req).await,
                _ => (
                    StatusCode::UNAUTHORIZED,
                    axum::Json(json!({
                        "status": "error",
                        "data": null,
                        "error": "Unauthorized — provide a valid Bearer token in the Authorization header"
                    }))
                ).into_response(),
            }
        }
    }
}

/// Request logging middleware: logs method, path, duration, status
pub async fn request_logger(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let start = Instant::now();

    let response = next.run(req).await;

    let duration = start.elapsed();
    let status = response.status();

    tracing::info!(
        method = %method,
        path = %path,
        status = %status,
        duration_ms = %duration.as_millis(),
        "request"
    );

    response
}

/// Error handling middleware: converts errors into standard JSON envelope
pub async fn error_handler(req: Request, next: Next) -> Response {
    let response = next.run(req).await;

    let status = response.status();

    // Only transform error responses
    if status.is_server_error() || status.is_client_error() {
        let (_parts, body) = response.into_parts();

        // Try to read body
        let body_bytes = match axum::body::to_bytes(body, usize::MAX).await {
            Ok(bytes) => bytes,
            Err(_) => {
                return (
                    status,
                    axum::Json(json!({
                        "status": "error",
                        "data": null,
                        "error": format!("HTTP {}", status.as_u16())
                    })),
                )
                    .into_response();
            }
        };

        let error_msg = String::from_utf8_lossy(&body_bytes);
        let error_msg = if error_msg.is_empty() {
            format!("HTTP {}", status.as_u16())
        } else {
            error_msg.to_string()
        };

        return (
            status,
            axum::Json(json!({
                "status": "error",
                "data": null,
                "error": error_msg
            })),
        )
            .into_response();
    }

    response
}


