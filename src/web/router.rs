use axum::{
    extract::Request,
    middleware,
    routing::{get, post},
    Router,
};

use super::state::AppState;
use super::handlers;
use super::middleware::{cors_layer, request_logger, error_handler, auth_middleware};

pub fn create_router(state: AppState) -> Router {
    // API routes — protected by auth middleware when auth is enabled
    let api_routes = Router::new()
        .route("/api/commands", get(handlers::commands::list_commands).post(handlers::commands::start_command))
        .route("/api/certificates", get(handlers::certificates::list_certificates))
        .route("/api/commands/:id/keys", post(handlers::keys::send_keys))
        .route("/api/commands/:id/kill", post(handlers::commands::kill_command))
        .route("/api/commands/:id/vtty", get(handlers::vtty::get_vtty_full))
        .route("/api/commands/:id/vtty/partial", get(handlers::vtty::get_vtty_partial))
        .route("/api/commands/:id/handles", get(handlers::handles::list_handles).post(handlers::handles::add_handle))
        .route("/api/shutdown", post(handlers::commands::shutdown));

    // Public routes — admin panel and static assets
    let public_routes = Router::new()
        .route("/admin", get(handlers::admin::admin_page))
        .route("/admin/*path", get(handlers::admin::admin_assets));

    // Auth middleware layer — injects auth requirement from state into extensions
    let auth_token = state.auth_token.clone();
    let api_routes = api_routes.route_layer(
        middleware::from_fn(move |req: Request, next| {
            let token = auth_token.clone();
            async move {
                // Inject auth token into request extensions for auth_middleware
                let mut req = req;
                req.extensions_mut().insert(token);
                auth_middleware(req, next).await
            }
        })
    );

    Router::new()
        .merge(api_routes)
        .merge(public_routes)
        .layer(cors_layer())
        .layer(middleware::from_fn(request_logger))
        .layer(middleware::from_fn(error_handler))
        .with_state(state)
}
