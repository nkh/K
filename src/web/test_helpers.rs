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
    use axum::routing::get;
    use axum::Router;
    use crate::web::handlers;
    use crate::web::state::AppState;
    use std::sync::Arc;

    let manager = Arc::new(crate::process::manager::CommandManager::new(config.clone()));
    let (shutdown_tx, _) = tokio::sync::broadcast::channel::<()>(1);
    let (vtty_events, _) = tokio::sync::broadcast::channel(16);
    let (log_events, _) = tokio::sync::broadcast::channel(16);
    let cert_store = std::sync::Arc::new(crate::web::certs::CertificateStore::new());
    let state = AppState::new(
        manager,
        shutdown_tx,
        None,
        cert_store,
        vtty_events,
        log_events,
    );

    let api_routes = Router::new()
        .route("/api/commands", get(handlers::commands::list_commands))
        .route("/api/info", get(handlers::commands::get_info));

    Router::new()
        .merge(api_routes)
        .with_state(state)
}