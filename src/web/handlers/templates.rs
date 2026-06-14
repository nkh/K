#![cfg(feature = "vrw")]

use axum::extract::State;
use axum::Json;
use serde_json::Value;

use crate::web::response::api_ok;
use crate::web::state::AppState;

/// GET /api/templates
///
/// Returns the list of command templates defined in the server configuration.
/// These are the `[[templates]]` entries from the config file.
pub async fn list_templates(State(state): State<AppState>) -> Json<Value> {
    let config = state.manager.config();
    let templates = &config.templates;

    let data: Vec<Value> = templates
        .iter()
        .map(|t| {
            serde_json::json!({
                "name": t.name,
                "cmd": t.cmd,
                "args": t.args,
                "env": t.env,
                "workdir": t.workdir,
                "certificate": t.certificate,
                "rows": t.rows,
                "cols": t.cols,
            })
        })
        .collect();

    api_ok(Value::Array(data))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::schema::{Config, TemplateConfig, TemplatesConfig};
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

    fn make_app_state_with_templates() -> AppState {
        let mut config = Config::default();
        config.binary_name = "test".to_string();
        config.templates = TemplatesConfig(vec![
            TemplateConfig {
                name: "my-template".to_string(),
                cmd: "htop".to_string(),
                args: Some("-d".to_string()),
                env: Some(vec!["KEY=val".to_string()]),
                workdir: Some("/tmp".to_string()),
                certificate: Some("my-cert".to_string()),
                rows: Some(50),
                cols: Some(200),
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
    async fn test_list_templates_empty() {
        let state = make_app_state();
        let result = list_templates(State(state)).await;
        assert_eq!(result.0["status"], "ok");
        assert_eq!(result.0["data"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_list_templates_with_data() {
        let state = make_app_state_with_templates();
        let result = list_templates(State(state)).await;
        assert_eq!(result.0["status"], "ok");
        let data = result.0["data"].as_array().unwrap();
        assert_eq!(data.len(), 1);
        assert_eq!(data[0]["name"], "my-template");
        assert_eq!(data[0]["cmd"], "htop");
        assert_eq!(data[0]["args"], "-d");
        assert_eq!(data[0]["rows"], 50);
        assert_eq!(data[0]["cols"], 200);
        assert_eq!(data[0]["certificate"], "my-cert");
    }
}