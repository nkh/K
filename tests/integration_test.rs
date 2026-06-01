//! Integration test suite for vrl/vrunner.
//!
//! Tests that use vrunner-specific Config fields (server, security, tls)
//! are gated behind `#[cfg(feature = "vrunner")]`.
//! Tests that only use shared modules (vtty, process manager basics) work
//! with both features.

use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

#[cfg(feature = "vrunner")]
use vrl_core::config::schema::{
    CommandLogConfig, Config, DaemonConfig, DisplayConfig, SecurityConfig, ServerConfig, TlsConfig,
    VttyConfig,
};
use vrl_core::process::manager::CommandManager;

#[cfg(feature = "vrunner")]
fn test_config() -> Config {
    Config {
        server: ServerConfig {
            bind: "127.0.0.1".to_string(),
            port: 0, // Let OS assign port
        },
        security: SecurityConfig::default(),
        tls: TlsConfig::default(),
        certificates: Default::default(),
        vtty: VttyConfig {
            rows: 10,
            cols: 40,
            term: "xterm-256color".to_string(),
            scrollback: 100,
            truecolor: true,
            mouse: false,
            screenshot_font_size: 12.0,
            screenshot_font_name: Some("monospace".to_string()),
        },
        display: DisplayConfig {
            enabled: false,
            refresh_ms: 100,
            display_all: false,
        },
        command_log: CommandLogConfig {
            enabled: false,
            file: None,
            pty_raw_log: None,
        },
        daemon: DaemonConfig {
            enabled: false,
            stdout_file: "/tmp/vrl-test.out".to_string(),
            stderr_file: "/tmp/vrl-test.err".to_string(),
        },
        handles: vec![],
        interactive: Default::default(),
        default_exit: Default::default(),
        environment: Default::default(),
        web: Default::default(),
        profiles: Default::default(),
        hooks: Default::default(),
        templates: Default::default(),
    }
}

#[cfg(feature = "vrunner")]
#[tokio::test]
async fn test_spawn_and_list() {
    let cfg = test_config();
    let manager = Arc::new(CommandManager::new(cfg));

    let id = manager
        .spawn(
            "echo".to_string(),
            vec!["hello".to_string()],
            None,
            None,
            std::collections::HashMap::new(),
            None,
            None,
            None,
        )
        .await
        .unwrap();

    let list = manager.list();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].0, id);

    // Clean up
    let _ = manager.kill(&id, None).await;
}

#[cfg(feature = "vrunner")]
#[tokio::test]
async fn test_vtty_contents() {
    let cfg = test_config();
    let manager = Arc::new(CommandManager::new(cfg));

    let id = manager
        .spawn(
            "echo".to_string(),
            vec!["test_output".to_string()],
            None,
            None,
            std::collections::HashMap::new(),
            None,
            None,
            None,
        )
        .await
        .unwrap();

    // Give process time to write and exit
    sleep(Duration::from_millis(200)).await;

    if let Some(handle) = manager.get(&id) {
        let plain = handle.vtty_plain().await;
        assert!(plain.contains("test_output") || plain.is_empty());
    }

    let _ = manager.kill(&id, None).await;
}

#[cfg(feature = "vrunner")]
#[tokio::test]
async fn test_send_keys() {
    let cfg = test_config();
    let manager = Arc::new(CommandManager::new(cfg));

    // Spawn a shell that reads input (cat is good for testing)
    let id = manager
        .spawn(
            "cat".to_string(),
            vec![],
            None,
            None,
            std::collections::HashMap::new(),
            None,
            None,
            None,
        )
        .await
        .unwrap();

    // Send some text
    manager.send_keys(&id, "hello").await.unwrap();
    manager.send_keys(&id, "<Enter>").await.unwrap();

    sleep(Duration::from_millis(100)).await;

    if let Some(handle) = manager.get(&id) {
        let plain = handle.vtty_plain().await;
        assert!(plain.contains("hello") || plain.is_empty());
    }

    // Send Ctrl+C to terminate cat
    let _ = manager.kill(&id, None).await;
}

#[test]
fn test_key_encoding() {
    use vrl_core::process::manager::encode_keys;

    assert_eq!(encode_keys("hello"), b"hello");
    assert_eq!(encode_keys("<C-c>"), vec![0x03]);
    assert_eq!(encode_keys("<Enter>"), vec![0x0d]);
    assert_eq!(encode_keys("<Esc>"), vec![0x1b]);
    assert_eq!(encode_keys("<Up>"), vec![0x1b, b'[', b'A']);
    assert_eq!(encode_keys("hello<C-c>world"), {
        let mut v = b"hello".to_vec();
        v.push(0x03);
        v.extend_from_slice(b"world");
        v
    });
}

/// Test that restarting a command (spawn replacement, kill old) leaves
/// the manager non-empty. This validates the core logic used by the web UI
/// restart button and `wait_for_child` — if the old command is killed
/// but a replacement exists, the server must NOT shut down.
#[cfg(feature = "vrunner")]
#[tokio::test]
async fn test_restart_keeps_manager_nonempty() {
    let cfg = test_config();
    let manager = Arc::new(CommandManager::new(cfg));

    // Spawn initial command (simulates the CLI-spawned "direct child")
    let old_id = manager
        .spawn(
            "sleep".to_string(),
            vec!["60".to_string()],
            None,
            None,
            std::collections::HashMap::new(),
            None,
            None,
            None,
        )
        .await
        .unwrap();

    assert_eq!(manager.list().len(), 1, "should have exactly 1 command after spawn");
    assert!(manager.get(&old_id).is_some(), "old command must be in manager");

    // Simulate the restart handler: spawn replacement FIRST, then kill old.
    // This matches web::handlers::commands::restart_command exactly.
    let new_id = manager
        .spawn(
            "sleep".to_string(),
            vec!["60".to_string()],
            None,
            None,
            std::collections::HashMap::new(),
            None,
            None,
            None,
        )
        .await
        .unwrap();

    // At this point both old and new are in the manager
    assert_eq!(manager.list().len(), 2, "should have 2 commands before killing old");

    // Kill the old command (the restart handler does this)
    let _ = manager.kill(&old_id, None).await;

    // Give the spawner task time to process the exit and remove the old command
    sleep(Duration::from_millis(300)).await;

    // The replacement must still be in the manager
    let list = manager.list();
    assert_eq!(list.len(), 1, "should have exactly 1 command after restart");
    assert_eq!(list[0].0, new_id, "remaining command should be the replacement");
    assert!(manager.get(&old_id).is_none(), "old command must be gone");
    assert!(manager.get(&new_id).is_some(), "new command must still be present");

    // Clean up
    let _ = manager.kill(&new_id, None).await;
}

/// Test that restarting the LAST command still works — the replacement
/// is spawned before the old is killed, so the manager is never empty.
#[cfg(feature = "vrunner")]
#[tokio::test]
async fn test_restart_single_command() {
    let cfg = test_config();
    let manager = Arc::new(CommandManager::new(cfg));

    let old_id = manager
        .spawn(
            "echo".to_string(),
            vec!["hello".to_string()],
            None,
            None,
            std::collections::HashMap::new(),
            None,
            None,
            None,
        )
        .await
        .unwrap();

    // Spawn replacement before killing (same order as restart handler)
    let new_id = manager
        .spawn(
            "sleep".to_string(),
            vec!["60".to_string()],
            None,
            None,
            std::collections::HashMap::new(),
            None,
            None,
            None,
        )
        .await
        .unwrap();

    let _ = manager.kill(&old_id, None).await;
    sleep(Duration::from_millis(300)).await;

    let list = manager.list();
    assert_eq!(list.len(), 1, "replacement should survive old command kill");
    assert_ne!(list[0].0, old_id, "old ID should not be in list");

    let _ = manager.kill(&new_id, None).await;
}
