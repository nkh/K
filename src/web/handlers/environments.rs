#![cfg(feature = "vrw")]

use axum::extract::State;
use axum::Json;
use serde_json::Value;

use crate::web::response::api_ok;
use crate::web::state::AppState;

/// GET /api/environments
///
/// Returns the list of workspace environments defined in the server
/// configuration.  These are the `[[environments]]` entries from the
/// config file.
pub async fn list_environments(State(state): State<AppState>) -> Json<Value> {
    let config = state.manager.config();
    let environments = &config.environments;

    let data: Vec<Value> = environments
        .iter()
        .map(|e| {
            let panels: Vec<Value> = e
                .panels
                .iter()
                .map(|p| {
                    let cmds: Vec<Value> = p
                        .commands
                        .iter()
                        .map(|c| {
                            serde_json::json!({
                                "cmd": c.cmd,
                                "args": c.args,
                                "workdir": c.workdir,
                                "certificate": c.certificate,
                                "rows": c.rows,
                                "cols": c.cols,
                                "retain_on_exit": c.retain_on_exit,
                            })
                        })
                        .collect();
                    serde_json::json!({
                        "title": p.title,
                        "server": p.server,
                        "token": p.token,
                        "server_label": p.server_label,
                        "commands": cmds,
                    })
                })
                .collect();
            serde_json::json!({
                "name": e.name,
                "description": e.description,
                "layout": e.layout,
                "auto_start": e.auto_start,
                "default_server": e.default_server,
                "panels": panels,
            })
        })
        .collect();

    api_ok(Value::Array(data))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::schema::{Config, EnvironmentsConfig, WorkspaceEnvironment, EnvironmentPanel, EnvironmentCommand};
    use crate::process::manager::CommandManager;
    use crate::web::certs::CertificateStore;
    use crate::web::state::AppState;
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

    fn make_app_state_with_environments() -> AppState {
        let mut config = Config::default();
        config.binary_name = "test".to_string();
        config.environments = EnvironmentsConfig(vec![
            WorkspaceEnvironment {
                name: "dev-env".to_string(),
                description: Some("Development workspace".to_string()),
                layout: Some("horizontal".to_string()),
                auto_start: Some(true),
                default_server: Some("http://localhost:9090".to_string()),
                default_token: None,
                panels: vec![
                    EnvironmentPanel {
                        title: Some("Main".to_string()),
                        server: Some("http://localhost:9090".to_string()),
                        token: Some("tok123".to_string()),
                        server_label: Some("local".to_string()),
                        commands: vec![
                            EnvironmentCommand {
                                cmd: "htop".to_string(),
                                args: Some("-d".to_string()),
                                workdir: None,
                                certificate: None,
                                rows: None,
                                cols: None,
                                retain_on_exit: Some(true),
                            },
                        ],
                    },
                ],
            },
        ]);
        let manager = Arc::new(CommandManager::new(config));
        let (shutdown_tx, _) = tokio::sync::broadcast::channel::<()>(1);
        let cert_store = Arc::new(CertificateStore::new());
        let (vtty_tx, _) = tokio::sync::broadcast::channel::<(String, String)>(16);
        let (log_tx, _) = tokio::sync::broadcast::channel::<String>(16);
        AppState::new(manager, shutdown_tx, None, cert_store, vtty_tx, log_tx)
    }

    #[tokio::test]
    async fn test_list_environments_empty() {
        let state = make_app_state();
        let result = list_environments(State(state)).await;
        assert_eq!(result.0["status"], "ok");
        assert_eq!(result.0["data"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_list_environments_with_data() {
        let state = make_app_state_with_environments();
        let result = list_environments(State(state)).await;
        assert_eq!(result.0["status"], "ok");
        let data = result.0["data"].as_array().unwrap();
        assert_eq!(data.len(), 1);
        assert_eq!(data[0]["name"], "dev-env");
        assert_eq!(data[0]["description"], "Development workspace");
        assert_eq!(data[0]["layout"], "horizontal");
        assert_eq!(data[0]["auto_start"], true);
        assert_eq!(data[0]["default_server"], "http://localhost:9090");
        let panels = data[0]["panels"].as_array().unwrap();
        assert_eq!(panels.len(), 1);
        assert_eq!(panels[0]["title"], "Main");
        assert_eq!(panels[0]["server"], "http://localhost:9090");
        assert_eq!(panels[0]["token"], "tok123");
        assert_eq!(panels[0]["server_label"], "local");
        let cmds = panels[0]["commands"].as_array().unwrap();
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0]["cmd"], "htop");
        assert_eq!(cmds[0]["args"], "-d");
        assert_eq!(cmds[0]["retain_on_exit"], true);
    }
}