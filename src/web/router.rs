#![cfg(feature = "vrw")]

use axum::{
    extract::Request,
    middleware,
    routing::{delete, get, post},
    Router,
};

use super::handlers;
use super::middleware::{auth_middleware, cors_layer, request_logger};
use super::state::AppState;
use crate::config::schema::CorsConfig;

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
        .route(
            "/api/completions",
            get(handlers::commands::tab_complete),
        )
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
        .route(
            "/api/commands/kill-all",
            post(handlers::commands::kill_all_commands),
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
        // Viewer token — authenticated "Open in New Tab"
        .route("/api/viewer/:id", get(handlers::share::create_viewer_token))
        .route("/api/shutdown", post(handlers::commands::shutdown));

    // Public routes — admin panel, share pages, and static assets
    let public_routes = Router::new()
        .route("/", get(handlers::admin::admin_page))
        .route("/admin", get(handlers::admin::admin_page))
        .route("/admin/*path", get(handlers::admin::admin_assets))
        .route("/favicon.ico", get(handlers::admin::admin_favicon))
        .route("/share/:token", get(handlers::admin::share_page))
        .route("/viewer/:token", get(handlers::admin::viewer_page))
        // Share WebSocket — public, authenticated via share token in URL
        .route("/api/share/:token/ws", get(handlers::share::ws_share_stream))
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
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use crate::config::schema::Config;
    use crate::process::manager::CommandManager;
    use crate::web::certs::CertificateStore;
    use crate::web::handlers::commands::start_command;
    use crate::web::handlers::commands::list_commands;
    use crate::web::handlers::commands::get_snapshot;
    use crate::web::handlers::commands::get_info;
    use crate::web::state::AppState;
    use axum::extract::State as AxumState;
    use axum::Json;
    use serde_json::json;
    use std::sync::Arc;

    fn make_app_state() -> AppState {
        let mut config = Config::default();
        config.binary_name = "test".to_string();
        let manager = Arc::new(CommandManager::new(config));
        let (shutdown_tx, _) = tokio::sync::broadcast::channel::<()>(1);
        let cert_store = Arc::new(CertificateStore::new());
        let (vtty_tx, _) = tokio::sync::broadcast::channel::<(String, String)>(16);
        let (log_tx, _) = tokio::sync::broadcast::channel::<String>(16);
        AppState::new(manager, shutdown_tx, None, cert_store, vtty_tx, log_tx)
    }

    #[tokio::test]
    async fn test_spawn_echo_through_handler() {
        let state = make_app_state();
        let body = json!({"cmd": "echo", "args": ["hello"]});
        let result = start_command(AxumState(state), Json(body)).await;
        assert_eq!(result.0["status"], "ok", "Spawn failed: {}", result.0);
        assert!(result.0["data"]["id"].is_string(), "Missing id: {}", result.0);
        let pid = result.0["data"]["pid"].as_u64().unwrap_or(0);
        assert!(pid > 0, "PID should be > 0: {}", result.0);
    }

    #[tokio::test]
    async fn test_spawn_then_list_shows_command() {
        let state = make_app_state();
        // Spawn
        let body = json!({"cmd": "echo", "args": ["integration_test"]});
        let result = start_command(AxumState(state.clone()), Json(body)).await;
        let id = result.0["data"]["id"].as_str().unwrap().to_string();

        // List
        let result = list_commands(AxumState(state)).await;
        assert_eq!(result.0["status"], "ok");
        let cmds = result.0["data"].as_array().unwrap();
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0]["id"], id);
        assert_eq!(cmds[0]["name"], "echo");
    }

    #[tokio::test]
    async fn test_spawn_then_snapshot_includes_command() {
        let state = make_app_state();
        // Spawn
        let body = json!({"cmd": "echo", "args": ["snapshot_test"]});
        let result = start_command(AxumState(state.clone()), Json(body)).await;
        let spawned_id = result.0["data"]["id"].as_str().unwrap().to_string();
        // Snapshot should include the command
        let result = get_snapshot(AxumState(state)).await;
        assert_eq!(result.0["status"], "ok");
        let cmds = result.0["data"]["commands"].as_array().unwrap();
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0]["id"], spawned_id);
    }

    #[tokio::test]
    async fn test_get_info_works() {
        let state = make_app_state();
        let result = get_info(AxumState(state)).await;
        assert_eq!(result.0["status"], "ok");
        assert_eq!(result.0["data"]["command_count"], 0);
        assert_eq!(result.0["data"]["auth_enabled"], false);
    }

    #[tokio::test]
    async fn test_spawn_missing_cmd_returns_error() {
        let state = make_app_state();
        let body = json!({"cmd": ""});
        let result = start_command(AxumState(state), Json(body)).await;
        assert_eq!(result.0["status"], "error");
        assert!(result.0["error"].as_str().unwrap().contains("Missing"));
    }

    #[tokio::test]
    async fn test_spawn_nonexistent_returns_error() {
        let state = make_app_state();
        let body = json!({"cmd": "nonexistent_binary_xyz_12345"});
        let result = start_command(AxumState(state), Json(body)).await;
        assert_eq!(result.0["status"], "error");
    }

    #[tokio::test]
    async fn test_spawn_with_env_and_dir() {
        let state = make_app_state();
        let body = json!({
            "cmd": "sh",
            "args": ["-c", "echo $TEST_VAR"],
            "env": {"TEST_VAR": "it_works"},
            "dir": "/tmp"
        });
        let result = start_command(AxumState(state), Json(body)).await;
        assert_eq!(result.0["status"], "ok", "Spawn failed: {}", result.0);
    }

    #[tokio::test]
    async fn test_spawn_with_dimensions() {
        let state = make_app_state();
        let body = json!({
            "cmd": "echo",
            "args": ["dim_test"],
            "rows": 40,
            "cols": 120
        });
        let result = start_command(AxumState(state), Json(body)).await;
        assert_eq!(result.0["status"], "ok", "Spawn failed: {}", result.0);
        assert!(result.0["data"]["pid"].as_u64().unwrap_or(0) > 0);
    }

    #[tokio::test]
    async fn test_spawn_invalid_dimensions_returns_error() {
        let state = make_app_state();
        let body = json!({
            "cmd": "echo",
            "args": [],
            "rows": 0,
            "cols": 80
        });
        let result = start_command(AxumState(state), Json(body)).await;
        assert_eq!(result.0["status"], "error");
        assert!(result.0["error"].as_str().unwrap().contains("Invalid dimensions"));
    }

    #[tokio::test]
    async fn test_spawn_invalid_dir_returns_error() {
        let state = make_app_state();
        let body = json!({
            "cmd": "echo",
            "args": [],
            "dir": "/nonexistent_dir_xyz_12345"
        });
        let result = start_command(AxumState(state), Json(body)).await;
        assert_eq!(result.0["status"], "error");
        assert!(result.0["error"].as_str().unwrap().contains("does not exist"));
    }

    #[tokio::test]
    async fn test_spawn_retain_on_exit() {
        let state = make_app_state();
        let body = json!({
            "cmd": "echo",
            "args": ["retain_test"],
            "retain_on_exit": true
        });
        let result = start_command(AxumState(state.clone()), Json(body)).await;
        assert_eq!(result.0["status"], "ok", "Spawn failed: {}", result.0);
        // Verify retain_on_exit is set
        let result = list_commands(AxumState(state)).await;
        let cmds = result.0["data"].as_array().unwrap();
        assert_eq!(cmds[0]["exit"]["retain_on_exit"], true);
    }
}