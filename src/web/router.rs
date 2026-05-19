use axum::{
    middleware,
    routing::{get, post},
    Router,
};

use super::state::AppState;
use super::handlers;
use super::middleware::{cors_layer, request_logger, error_handler};

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/api/commands", get(handlers::commands::list_commands).post(handlers::commands::start_command))
        .route("/api/commands/:id/keys", post(handlers::keys::send_keys))
        .route("/api/commands/:id/kill", post(handlers::commands::kill_command))
        .route("/api/commands/:id/vtty", get(handlers::vtty::get_vtty_full))
        .route("/api/commands/:id/vtty/partial", get(handlers::vtty::get_vtty_partial))
        .route("/api/commands/:id/handles", get(handlers::handles::list_handles).post(handlers::handles::add_handle))
        .route("/api/shutdown", post(handlers::commands::shutdown))
        .route("/admin", get(handlers::admin::admin_page))
        .route("/admin/*path", get(handlers::admin::admin_assets))
        .layer(cors_layer())
        .layer(middleware::from_fn(request_logger))
        .layer(middleware::from_fn(error_handler))
        .with_state(state)
}
