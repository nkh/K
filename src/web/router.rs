#![cfg(feature = "vrw")]
#![allow(dead_code, unused_imports)]
use axum::{
    extract::Request,
    middleware,
    routing::{delete, get, post},
    Router,
};

use super::handlers;
use super::middleware::{auth_middleware, cors_layer, error_handler, request_logger};
use super::state::AppState;
use crate::config::security::CorsConfig;

pub fn create_router(state: AppState, cors_config: &CorsConfig) -> Router {
    // API routes — protected by auth middleware when auth is enabled
    let api_routes = Router::new()
        .route(
            "/api/snapshot",
            get(handlers::commands::get_snapshot),
        )
        .route(
            "/api/commands",
            get(handlers::commands::list_commands).post(handlers::commands::start_command),
        )
        .route(
            "/api/commands/lookup/:name",
            get(handlers::commands::lookup_command),
        )
        .route(
            "/api/certificates",
            get(handlers::certificates::list_certificates),
        )
        .route("/api/info", get(handlers::commands::get_info))
        .route("/api/templates", get(handlers::templates::list_templates))
        .route("/api/environments", get(handlers::environments::list_environments))
        .route("/api/log", get(handlers::logs::get_log))
        // Peers — registration and discovery for multi-instance failover
        .route(
            "/api/peers",
            get(handlers::peers::list_peers).post(handlers::peers::register_peer),
        )
        .route(
            "/api/peers/:url",
            delete(handlers::peers::unregister_peer),
        )
        .route(
            "/api/commands/kill-pid/:pid",
            post(handlers::commands::kill_command_by_pid),
        )
        .route("/api/commands/:id/keys", post(handlers::keys::send_keys))
        .route("/api/commands/:id/mouse", post(handlers::keys::send_mouse))
        .route(
            "/api/commands/:id/kill",
            post(handlers::commands::kill_command),
        )
        .route(
            "/api/commands/:id/restart",
            post(handlers::commands::restart_command),
        )
        .route(
            "/api/commands/:id",
            delete(handlers::commands::purge_command),
        )
        .route(
            "/api/commands/:id/freeze",
            post(handlers::commands::freeze_command),
        )
        .route(
            "/api/commands/:id/thaw",
            post(handlers::commands::thaw_command),
        )
        .route(
            "/api/commands/:id/keep",
            post(handlers::commands::keep_command),
        )
        .route(
            "/api/commands/:id/unkeep",
            post(handlers::commands::unkeep_command),
        )
        .route("/api/commands/:id/vtty", get(handlers::vtty::get_vtty_full))
        .route(
            "/api/commands/:id/vtty/html",
            get(handlers::vtty::get_vtty_html),
        )
        .route(
            "/api/commands/:id/vtty/buffer",
            get(handlers::vtty::get_vtty_buffer),
        )
        .route(
            "/api/commands/:id/vtty/changed",
            get(handlers::vtty::vtty_changed),
        )
        .route(
            "/api/commands/:id/vtty/diff",
            get(handlers::vtty::get_vtty_diff),
        )
        .route(
            "/api/commands/:id/vtty/partial",
            get(handlers::vtty::get_vtty_partial),
        )
        .route(
            "/api/commands/:id/vtty/text",
            get(handlers::vtty::get_vtty_text),
        )
        .route(
            "/api/commands/:id/vtty/png",
            get(handlers::vtty::get_vtty_png),
        )
        .route(
            "/api/commands/:id/resize",
            post(handlers::vtty::resize_vtty),
        )
        .route(
            "/api/commands/:id/snapshot",
            post(handlers::commands::snapshot_command),
        )
        .route(
            "/api/commands/:id/snapshots",
            get(handlers::commands::list_snapshots),
        )
        .route(
            "/api/commands/:id/diff",
            post(handlers::commands::diff_command),
        )
        .route(
            "/api/commands/:id/snapshots/:name",
            delete(handlers::commands::delete_snapshot),
        )
        .route(
            "/api/commands/:id/handles",
            get(handlers::handles::list_handles).post(handlers::handles::add_handle),
        )
        .route(
            "/api/commands/:id/resources",
            get(handlers::resources::get_resources),
        )
        .route("/api/commands/:id/ws", get(handlers::ws::ws_vtty_stream))
        .route(
            "/api/commands/:id/share",
            post(handlers::share::create_share_token),
        )
        .route("/api/ws/logs", get(handlers::ws::ws_log_stream))
        .route("/api/share/:token", get(handlers::share::get_share))
        .route("/api/shutdown", post(handlers::commands::shutdown));

    // Public routes — admin panel and static assets
    let public_routes = Router::new()
        .route("/", get(handlers::admin::admin_page))
        .route("/admin", get(handlers::admin::admin_page))
        .route("/admin/*path", get(handlers::admin::admin_assets))
        .route("/favicon.ico", get(handlers::admin::admin_favicon))
        .route("/share/:token", get(handlers::admin::share_page))
        // Smart fallback: serves embedded static assets (style.css, app.js, etc.)
        // with correct MIME types, or index.html for command-name URL routing.
        .fallback(handlers::admin::smart_fallback);

    // Auth middleware layer — injects auth requirement from state into extensions
    let auth_token = state.auth_token.clone();
    let api_routes = api_routes.route_layer(middleware::from_fn(move |req: Request, next| {
        let token = auth_token.clone();
        async move {
            // Inject auth token into request extensions for auth_middleware
            let mut req = req;
            req.extensions_mut().insert(token);
            auth_middleware(req, next).await
        }
    }));

    Router::new()
        .merge(api_routes)
        .merge(public_routes)
        .layer(cors_layer(cors_config))
        .layer(middleware::from_fn(request_logger))
        .layer(middleware::from_fn(error_handler))
        .with_state(state)
}
