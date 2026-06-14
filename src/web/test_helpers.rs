/// Test helpers for vrw web handler integration tests.
///
/// Usage:
/// ```ignore
/// use crate::web::test_helpers::*;
///
/// let app = test_app(config);
/// let response = app.oneshot(request).await;
/// ```

#[cfg(all(feature = "vrw", test))]
pub fn test_app(config: crate::config::schema::Config) -> axum::Router {
    use axum::Router;
    use crate::web::handlers;
    use crate::web::middleware::{auth_middleware, cors_layer, error_handler, request_logger};
    use crate::web::state::AppState;
    use std::sync::Arc;

    let manager = Arc::new(crate::process::manager::CommandManager::new(config.clone()));
    let state = AppState {
        manager,
        auth_token: None,
        server_name: "test".into(),
        web_config: config.web.clone(),
        cors_config: config.security.cors.clone(),
        cert_pool: Default::default(),
    };

    let api_routes = Router::new()
        .route("/api/commands", get(handlers::commands::list_commands))
        .route("/api/info", get(handlers::commands::get_info));

    Router::new()
        .merge(api_routes)
        .layer(cors_layer(&config.security.cors))
        .layer(tower::ServiceBuilder::new().layer(error_handler))
        .with_state(state)
}