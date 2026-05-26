use axum::{
    extract::Request,
    http::header::AUTHORIZATION,
    middleware::Next,
    response::{IntoResponse, Response},
    http::StatusCode,
};
use serde_json::json;
use std::time::Instant;
use tracing;

/// CORS layer configuration
pub fn cors_layer() -> tower_http::cors::CorsLayer {
    tower_http::cors::CorsLayer::new()
        .allow_origin(tower_http::cors::Any)
        .allow_methods(tower_http::cors::Any)
        .allow_headers(tower_http::cors::Any)
}

/// Authentication middleware: validates Bearer token when auth is enabled.
/// When the auth_token extension is None (localhost default), all requests are allowed.
pub async fn auth_middleware(
    req: Request,
    next: Next,
) -> Response {
    // Extract the auth token from extensions (set by the router layer)
    let token = req.extensions().get::<Option<String>>().cloned().flatten();

    match token {
        None => {
            // No auth required — pass through
            next.run(req).await
        }
        Some(expected) => {
            // Auth required — validate Bearer token
            let provided = req.headers()
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
                    }))
                ).into_response();
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
            }))
        ).into_response();
    }

    response
}
