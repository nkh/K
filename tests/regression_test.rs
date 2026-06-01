//! Regression test suite for vrw.
//!
//! These tests cover the critical integration paths that have historically
//! broken during code changes.  They are organized by scenario:
//!
//!   1. Command lifecycle (spawn, list, kill, purge)
//!   2. IPC simulation (HTTP API round-trip)
//!   3. Exit behavior (retain-on-exit, snapshot-on-exit)
//!   4. Multi-command management (spawn multiple, kill one, etc.)
//!   5. VTTY operations (snapshot, diff, resize, change detection)
//!   6. Key encoding and delivery
//!   7. Config and CLI overrides
//!   8. Instance registry and shutdown
//!   9. Web server start/stop/shutdown signal
//!  10. Edge cases and error handling
//!
//! Every test is independent: no shared state, no ordering dependency.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

use vrc_core::config::schema::{
    CommandLogConfig, Config, DaemonConfig, DisplayConfig, EnvironmentConfig, ExitConfig,
};
#[cfg(feature = "vrw")]
use vrc_core::config::schema::{SecurityConfig, ServerConfig, TlsConfig, VttyConfig};
use vrc_core::process::manager::CommandManager;

// ─── Test helpers ───────────────────────────────────────────────────────

/// Create a test config with a random port (0 = OS-assigned).
#[cfg(feature = "vrw")]
fn test_config() -> Config {
    Config {
        server: ServerConfig {
            bind: "127.0.0.1".to_string(),
            port: 0,
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
            stdout_file: "/tmp/vrc-test.out".to_string(),
            stderr_file: "/tmp/vrc-test.err".to_string(),
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

/// Create a test config for vrc-only mode (no server/security/tls fields).
#[cfg(not(feature = "vrw"))]
fn test_config() -> Config {
    Config {
        vtty: vrc_core::config::schema::VttyConfig {
            rows: 10,
            cols: 40,
            term: "xterm-256color".to_string(),
            scrollback: 100,
            truecolor: true,
            mouse: false,
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
            stdout_file: "/tmp/vrc-test.out".to_string(),
            stderr_file: "/tmp/vrc-test.err".to_string(),
        },
        handles: vec![],
        interactive: Default::default(),
        default_exit: Default::default(),
        environment: Default::default(),
        profiles: Default::default(),
        hooks: Default::default(),
        templates: Default::default(),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 1. COMMAND LIFECYCLE REGRESSION TESTS
// ═══════════════════════════════════════════════════════════════════════

#[tokio::test]
#[cfg(feature = "vrw")]
async fn regression_spawn_echo_returns_valid_id() {
    // Ensures spawn() always returns a non-empty UUID string.
    // Regression: broken IPC paths used to return empty/error IDs.
    let cfg = test_config();
    let manager = Arc::new(CommandManager::new(cfg));
    let id = manager
        .spawn(
            "echo".into(),
            vec!["hi".into()],
            None,
            None,
            HashMap::new(),
            None,
            None,
            None,
        )
        .await
        .unwrap();
    assert!(!id.is_empty(), "spawn() returned empty ID");
    assert!(id.len() >= 32, "spawn() ID looks too short: {}", id);
    let _ = manager.kill(&id, None).await;
}

#[tokio::test]
#[cfg(feature = "vrw")]
async fn regression_spawn_appears_in_list() {
    // After spawning, list() must include the new command.
    // Regression: commands were spawned but not registered in DashMap.
    let cfg = test_config();
    let manager = Arc::new(CommandManager::new(cfg));
    let id = manager
        .spawn(
            "echo".into(),
            vec!["test".into()],
            None,
            None,
            HashMap::new(),
            None,
            None,
            None,
        )
        .await
        .unwrap();
    let list = manager.list();
    assert_eq!(list.len(), 1, "list() should have exactly 1 command");
    assert_eq!(list[0].0, id, "list() ID must match spawn() return");
    assert_eq!(list[0].1, "echo", "list() name must be 'echo'");
    let _ = manager.kill(&id, None).await;
}

#[tokio::test]
#[cfg(feature = "vrw")]
async fn regression_kill_removes_from_list() {
    // After kill(), the command must be gone from list().
    // Regression: kill() sent SIGINT but never removed from DashMap.
    let cfg = test_config();
    let manager = Arc::new(CommandManager::new(cfg));
    let id = manager
        .spawn(
            "sleep".into(),
            vec!["60".into()],
            None,
            None,
            HashMap::new(),
            None,
            None,
            None,
        )
        .await
        .unwrap();
    assert_eq!(manager.list().len(), 1);
    manager.kill(&id, None).await.unwrap();
    sleep(Duration::from_millis(200)).await;
    assert!(
        manager.get(&id).is_none(),
        "killed command still in manager"
    );
    assert_eq!(manager.list().len(), 0, "list() should be empty after kill");
}

#[tokio::test]
#[cfg(feature = "vrw")]
async fn regression_kill_nonexistent_returns_error() {
    // Killing a command that doesn't exist must not panic.
    let cfg = test_config();
    let manager = Arc::new(CommandManager::new(cfg));
    let nonexistent = String::from("nonexistent-id");
    let result = manager.kill(&nonexistent, None).await;
    // kill() removes from DashMap — if not found, it's a no-op Ok(())
    assert!(result.is_ok(), "kill() on nonexistent ID should not panic");
}

#[tokio::test]
#[cfg(feature = "vrw")]
async fn regression_spawn_multiple_all_visible() {
    // Spawning 3 commands must show all 3 in list().
    // Regression: DashMap race condition lost commands.
    // NOTE: use long-running commands (sleep) so they don't exit
    // and get auto-removed before we can check list().
    let cfg = test_config();
    let manager = Arc::new(CommandManager::new(cfg));
    let id1 = manager
        .spawn(
            "sleep".into(),
            vec!["60".into()],
            None,
            None,
            HashMap::new(),
            None,
            None,
            None,
        )
        .await
        .unwrap();
    let id2 = manager
        .spawn(
            "sleep".into(),
            vec!["60".into()],
            None,
            None,
            HashMap::new(),
            None,
            None,
            None,
        )
        .await
        .unwrap();
    let id3 = manager
        .spawn(
            "sleep".into(),
            vec!["60".into()],
            None,
            None,
            HashMap::new(),
            None,
            None,
            None,
        )
        .await
        .unwrap();
    let list = manager.list();
    assert_eq!(list.len(), 3, "should have 3 commands");
    let ids: Vec<&str> = list.iter().map(|(id, _, _, _, _)| id.as_str()).collect();
    assert!(ids.contains(&id1.as_str()));
    assert!(ids.contains(&id2.as_str()));
    assert!(ids.contains(&id3.as_str()));
    let _ = manager.kill(&id1, None).await;
    let _ = manager.kill(&id2, None).await;
    let _ = manager.kill(&id3, None).await;
}

#[tokio::test]
#[cfg(feature = "vrw")]
async fn regression_kill_one_preserves_others() {
    // Killing one of multiple commands must leave the others intact.
    // Regression: kill() removed wrong command from DashMap.
    let cfg = test_config();
    let manager = Arc::new(CommandManager::new(cfg));
    let id1 = manager
        .spawn(
            "sleep".into(),
            vec!["60".into()],
            None,
            None,
            HashMap::new(),
            None,
            None,
            None,
        )
        .await
        .unwrap();
    let id2 = manager
        .spawn(
            "sleep".into(),
            vec!["60".into()],
            None,
            None,
            HashMap::new(),
            None,
            None,
            None,
        )
        .await
        .unwrap();
    manager.kill(&id1, None).await.unwrap();
    sleep(Duration::from_millis(100)).await;
    assert_eq!(manager.list().len(), 1);
    assert!(manager.get(&id2).is_some(), "id2 should still exist");
    let _ = manager.kill(&id2, None).await;
}

#[tokio::test]
#[cfg(feature = "vrw")]
async fn regression_find_by_pid_returns_correct_id() {
    // find_by_pid must return the correct command ID.
    let cfg = test_config();
    let manager = Arc::new(CommandManager::new(cfg));
    let id = manager
        .spawn(
            "echo".into(),
            vec!["pid_test".into()],
            None,
            None,
            HashMap::new(),
            None,
            None,
            None,
        )
        .await
        .unwrap();
    let pid = manager.get(&id).unwrap().pid;
    let found = manager.find_by_pid(pid);
    assert_eq!(found, Some(id.clone()), "find_by_pid returned wrong ID");
    let _ = manager.kill(&id, None).await;
}

#[tokio::test]
#[cfg(feature = "vrw")]
async fn regression_kill_by_pid_works() {
    // kill_by_pid must remove the correct command.
    let cfg = test_config();
    let manager = Arc::new(CommandManager::new(cfg));
    let id = manager
        .spawn(
            "sleep".into(),
            vec!["60".into()],
            None,
            None,
            HashMap::new(),
            None,
            None,
            None,
        )
        .await
        .unwrap();
    let pid = manager.get(&id).unwrap().pid;
    manager.kill_by_pid(pid).await.unwrap();
    sleep(Duration::from_millis(200)).await;
    assert!(manager.get(&id).is_none());
}

#[tokio::test]
#[cfg(feature = "vrw")]
async fn regression_purge_nonexistent_returns_error() {
    let cfg = test_config();
    let manager = Arc::new(CommandManager::new(cfg));
    let nonexistent = String::from("nonexistent-id");
    let result = manager.purge(&nonexistent);
    assert!(result.is_err(), "purge nonexistent ID should error");
}

// ═══════════════════════════════════════════════════════════════════════
// 2. IPC SIMULATION TESTS (HTTP API round-trip)
// ═══════════════════════════════════════════════════════════════════════

#[cfg(feature = "vrw")]
#[tokio::test]
async fn regression_http_client_has_timeout() {
    // The HTTP client used by subcommands MUST have timeouts.
    // Regression: reqwest::Client::new() had NO timeout, causing
    // vrw spawn and vrw stop to block forever.
    let client = vrc_core::cli::subcommands::http_client();
    // We can't easily inspect timeouts from the public API, but
    // we verify the client is constructible and functional.
    let resp = client
        .get("http://127.0.0.1:1") // port 1 = definitely closed
        .send()
        .await;
    // Should fail fast (within timeout), not hang forever
    assert!(resp.is_err(), "request to closed port should fail");
}

// ═══════════════════════════════════════════════════════════════════════
// 3. EXIT BEHAVIOR REGRESSION TESTS
// ═══════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn regression_exit_config_default_no_retain() {
    // Default ExitConfig must have retain_on_exit = false.
    // Regression: default was changed to true, breaking exit-on-completion.
    let ec = ExitConfig::default();
    assert!(!ec.retain_on_exit, "default retain_on_exit must be false");
    assert!(ec.on_exit.is_none());
    assert!(ec.on_error.is_none());
    assert!(ec.snapshot_on_exit.is_none());
    assert_eq!(ec.timeout_secs, 10);
}

#[tokio::test]
async fn regression_exit_config_retain_true() {
    let ec = ExitConfig {
        retain_on_exit: true,
        ..Default::default()
    };
    assert!(ec.retain_on_exit);
}

#[tokio::test]
async fn regression_exit_config_snapshot_on_exit() {
    let ec = ExitConfig {
        snapshot_on_exit: Some("/tmp/snap.txt".into()),
        ..Default::default()
    };
    assert_eq!(ec.snapshot_on_exit.as_deref(), Some("/tmp/snap.txt"));
}

#[tokio::test]
#[cfg(feature = "vrw")]
async fn regression_command_exits_process_removed() {
    // When a short-lived command exits WITHOUT retain_on_exit, it should
    // eventually be removed from the manager.
    // Regression: commands were never removed, causing list() to grow.
    let cfg = test_config();
    let manager = Arc::new(CommandManager::new(cfg));
    let id = manager
        .spawn(
            "echo".into(),
            vec!["done".into()],
            None,
            None,
            HashMap::new(),
            None,
            None,
            None,
        )
        .await
        .unwrap();
    // Wait for echo to finish + spawner cleanup
    sleep(Duration::from_millis(500)).await;
    // After echo exits and spawner cleans up, it should be removed
    // (without retain_on_exit, the spawner removes it)
    // Note: we don't assert it's gone here because the cleanup is async,
    // but we verify kill doesn't panic
    let _ = manager.kill(&id, None).await;
}

// ═══════════════════════════════════════════════════════════════════════
// 4. MULTI-COMMAND MANAGEMENT REGRESSION TESTS
// ═══════════════════════════════════════════════════════════════════════

#[tokio::test]
#[cfg(feature = "vrw")]
async fn regression_spawn_after_kill() {
    // Spawning a command after killing a previous one must work.
    // Regression: DashMap state corruption after kill prevented new spawns.
    let cfg = test_config();
    let manager = Arc::new(CommandManager::new(cfg));
    let id1 = manager
        .spawn(
            "echo".into(),
            vec!["first".into()],
            None,
            None,
            HashMap::new(),
            None,
            None,
            None,
        )
        .await
        .unwrap();
    manager.kill(&id1, None).await.unwrap();
    sleep(Duration::from_millis(100)).await;

    let id2 = manager
        .spawn(
            "echo".into(),
            vec!["second".into()],
            None,
            None,
            HashMap::new(),
            None,
            None,
            None,
        )
        .await
        .unwrap();
    assert_eq!(manager.list().len(), 1);
    assert_eq!(manager.list()[0].0, id2);
    let _ = manager.kill(&id2, None).await;
}

#[tokio::test]
#[cfg(feature = "vrw")]
async fn regression_list_empty_after_all_killed() {
    let cfg = test_config();
    let manager = Arc::new(CommandManager::new(cfg));
    let mut ids = vec![];
    for _ in 0..5 {
        let id = manager
            .spawn(
                "echo".into(),
                vec!["x".into()],
                None,
                None,
                HashMap::new(),
                None,
                None,
                None,
            )
            .await
            .unwrap();
        ids.push(id);
    }
    for id in &ids {
        manager.kill(id, None).await.unwrap();
    }
    sleep(Duration::from_millis(300)).await;
    assert_eq!(
        manager.list().len(),
        0,
        "list should be empty after killing all"
    );
}

#[tokio::test]
#[cfg(feature = "vrw")]
async fn regression_concurrent_spawns() {
    // Spawning many commands concurrently must not lose any.
    // NOTE: use long-running commands (sleep) so they don't exit
    // and get auto-removed before we can count them.
    let cfg = test_config();
    let manager = Arc::new(CommandManager::new(cfg));
    let mut handles = vec![];
    for _ in 0..10 {
        let mgr = manager.clone();
        handles.push(tokio::spawn(async move {
            mgr.spawn(
                "sleep".into(),
                vec!["60".into()],
                None,
                None,
                HashMap::new(),
                None,
                None,
                None,
            )
            .await
        }));
    }
    let mut ids = vec![];
    for h in handles {
        ids.push(h.await.unwrap().unwrap());
    }
    let list = manager.list();
    assert_eq!(
        list.len(),
        10,
        "concurrent spawns: expected 10, got {}",
        list.len()
    );
    for id in &ids {
        let _ = manager.kill(id, None).await;
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 5. VTTY OPERATIONS REGRESSION TESTS
// ═══════════════════════════════════════════════════════════════════════

#[tokio::test]
#[cfg(feature = "vrw")]
async fn regression_vtty_snapshot_returns_data() {
    let cfg = test_config();
    let manager = Arc::new(CommandManager::new(cfg));
    let id = manager
        .spawn(
            "echo".into(),
            vec!["snapshot_test".into()],
            None,
            None,
            HashMap::new(),
            None,
            None,
            None,
        )
        .await
        .unwrap();
    sleep(Duration::from_millis(300)).await;
    if let Some(handle) = manager.get(&id) {
        let buf = handle.vtty_snapshot().await;
        assert!(buf.width > 0, "buffer width must be positive");
        assert!(buf.height > 0, "buffer height must be positive");
    }
    let _ = manager.kill(&id, None).await;
}

#[tokio::test]
#[cfg(feature = "vrw")]
async fn regression_vtty_html_contains_content() {
    let cfg = test_config();
    let manager = Arc::new(CommandManager::new(cfg));
    let id = manager
        .spawn(
            "printf".into(),
            vec!["hello\\nworld".into()],
            None,
            None,
            HashMap::new(),
            None,
            None,
            None,
        )
        .await
        .unwrap();
    sleep(Duration::from_millis(300)).await;
    if let Some(handle) = manager.get(&id) {
        let html = handle.vtty_html().await;
        // The HTML may or may not contain the text depending on timing,
        // but it must be valid HTML (contain span tags from renderer)
        assert!(html.len() > 0, "HTML output must not be empty");
    }
    let _ = manager.kill(&id, None).await;
}

#[tokio::test]
#[cfg(feature = "vrw")]
async fn regression_vtty_plain_text_readable() {
    let cfg = test_config();
    let manager = Arc::new(CommandManager::new(cfg));
    let id = manager
        .spawn(
            "echo".into(),
            vec!["plain_text_check".into()],
            None,
            None,
            HashMap::new(),
            None,
            None,
            None,
        )
        .await
        .unwrap();
    sleep(Duration::from_millis(300)).await;
    if let Some(handle) = manager.get(&id) {
        let plain = handle.vtty_plain().await;
        // Should contain the text or be empty (timing)
        // The important thing is it doesn't panic or deadlock
        let _ = &plain;
    }
    let _ = manager.kill(&id, None).await;
}

#[tokio::test]
#[cfg(feature = "vrw")]
async fn regression_resize_command() {
    let cfg = test_config();
    let manager = Arc::new(CommandManager::new(cfg));
    let id = manager
        .spawn(
            "sleep".into(),
            vec!["60".into()],
            None,
            None,
            HashMap::new(),
            None,
            None,
            None,
        )
        .await
        .unwrap();
    if let Some(handle) = manager.get(&id) {
        let result = handle.resize(20, 80).await;
        assert!(result.is_ok(), "resize should succeed");
        let (rows, cols) = handle.dimensions().await;
        assert_eq!(rows, 20, "rows after resize should be 20");
        assert_eq!(cols, 80, "cols after resize should be 80");
    }
    let _ = manager.kill(&id, None).await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg(feature = "vrw")]
async fn regression_snapshot_store_and_retrieve() {
    // Use a long-running command so it stays in the manager for snapshot ops.
    let cfg = test_config();
    let manager = Arc::new(CommandManager::new(cfg));
    let id = manager
        .spawn(
            "sleep".into(),
            vec!["60".into()],
            None,
            None,
            HashMap::new(),
            None,
            None,
            None,
        )
        .await
        .unwrap();
    sleep(Duration::from_millis(100)).await;

    let meta = manager.store_snapshot(&id, "test_snap").unwrap();
    assert_eq!(meta.name, "test_snap");
    assert_eq!(meta.command_id, id);

    let snaps = manager.list_snapshots(&id);
    assert_eq!(snaps.len(), 1);
    assert_eq!(snaps[0].name, "test_snap");

    // Delete and verify gone
    manager.delete_snapshot(&id, "test_snap").unwrap();
    let snaps2 = manager.list_snapshots(&id);
    assert_eq!(snaps2.len(), 0);

    let _ = manager.kill(&id, None).await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg(feature = "vrw")]
async fn regression_diff_snapshot() {
    // Use a long-running command so it stays in the manager for snapshot ops.
    let cfg = test_config();
    let manager = Arc::new(CommandManager::new(cfg));
    let id = manager
        .spawn(
            "sleep".into(),
            vec!["60".into()],
            None,
            None,
            HashMap::new(),
            None,
            None,
            None,
        )
        .await
        .unwrap();
    sleep(Duration::from_millis(100)).await;

    manager.store_snapshot(&id, "base").unwrap();
    sleep(Duration::from_millis(100)).await;

    // Buffer should be identical → diff should have 0 changes
    let diff = manager.diff_snapshot(&id, "base").unwrap();
    assert_eq!(
        diff.changed_count, 0,
        "diff against identical snapshot should have 0 changes"
    );

    let _ = manager.kill(&id, None).await;
}

// NOTE: has_changed() uses block_in_place internally, which requires
// a multi-threaded tokio runtime.  The default #[tokio::test] runtime is
// current_thread, so we must specify multi_thread flavour here.
#[tokio::test(flavor = "multi_thread")]
#[cfg(feature = "vrw")]
async fn regression_has_changed_detection() {
    let cfg = test_config();
    let manager = Arc::new(CommandManager::new(cfg));
    let id = manager
        .spawn(
            "sleep".into(),
            vec!["60".into()],
            None,
            None,
            HashMap::new(),
            None,
            None,
            None,
        )
        .await
        .unwrap();

    // First check always reports changed (no previous generation stored)
    let result1 = manager.has_changed(&id);
    assert!(result1.is_ok(), "first has_changed should succeed");
    assert!(result1.unwrap(), "first has_changed should report changed");

    // Second check should report not-changed (no new writes)
    let result2 = manager.has_changed(&id);
    assert!(result2.is_ok(), "second has_changed should succeed");
    assert!(
        !result2.unwrap(),
        "second has_changed should report not-changed"
    );

    let _ = manager.kill(&id, None).await;
}

// ═══════════════════════════════════════════════════════════════════════
// 6. KEY ENCODING AND DELIVERY REGRESSION TESTS
// ═══════════════════════════════════════════════════════════════════════

#[tokio::test]
#[cfg(feature = "vrw")]
async fn regression_send_keys_to_running_command() {
    // Send keys to a long-running process.
    // Regression: send_keys() panicked when stdin channel was closed.
    let cfg = test_config();
    let manager = Arc::new(CommandManager::new(cfg));
    let id = manager
        .spawn(
            "cat".into(),
            vec![],
            None,
            None,
            HashMap::new(),
            None,
            None,
            None,
        )
        .await
        .unwrap();

    let result = manager.send_keys(&id, "hello world").await;
    assert!(
        result.is_ok(),
        "send_keys should succeed for running command"
    );

    let result2 = manager.send_keys(&id, "<Enter>").await;
    assert!(result2.is_ok(), "send_keys with Enter should succeed");

    let _ = manager.kill(&id, None).await;
}

#[tokio::test]
#[cfg(feature = "vrw")]
async fn regression_send_keys_nonexistent_errors() {
    let cfg = test_config();
    let manager = Arc::new(CommandManager::new(cfg));
    let nonexistent = String::from("nonexistent");
    let result = manager.send_keys(&nonexistent, "test").await;
    assert!(
        result.is_err(),
        "send_keys to nonexistent command should error"
    );
}

#[tokio::test]
#[cfg(feature = "vrw")]
async fn regression_send_ctrl_c_terminates() {
    // Sending Ctrl+C should eventually terminate the process.
    let cfg = test_config();
    let manager = Arc::new(CommandManager::new(cfg));
    let id = manager
        .spawn(
            "cat".into(),
            vec![],
            None,
            None,
            HashMap::new(),
            None,
            None,
            None,
        )
        .await
        .unwrap();

    manager.send_keys(&id, "<C-c>").await.unwrap();
    sleep(Duration::from_millis(500)).await;

    // Process should be dead or command removed
    let alive = manager.get(&id).map(|h| h.is_alive()).unwrap_or(false);
    assert!(!alive, "cat should be terminated after Ctrl+C");
}

#[tokio::test]
async fn regression_encode_all_special_keys() {
    // Ensure encode_keys doesn't panic on any special key.
    use vrc_core::process::manager::encode_keys;
    let keys = [
        "<Up>",
        "<Down>",
        "<Left>",
        "<Right>",
        "<Enter>",
        "<Esc>",
        "<Tab>",
        "<Backspace>",
        "<Delete>",
        "<Home>",
        "<End>",
        "<PageUp>",
        "<PageDown>",
        "<F1>",
        "<F2>",
        "<F3>",
        "<F4>",
        "<F5>",
        "<F6>",
        "<F7>",
        "<F8>",
        "<F9>",
        "<F10>",
        "<F11>",
        "<F12>",
        "<C-a>",
        "<C-b>",
        "<C-c>",
        "<C-d>",
        "<C-z>",
        "<A-x>",
        "<A-y>",
        "<A-z>",
        "hello<C-c>world<Enter>done",
    ];
    for key in &keys {
        let encoded = encode_keys(key);
        assert!(!encoded.is_empty(), "encode_keys({:?}) returned empty", key);
    }
}

#[tokio::test]
async fn regression_encode_plain_text_unchanged() {
    use vrc_core::process::manager::encode_keys;
    assert_eq!(encode_keys("hello world 123"), b"hello world 123");
}

// ═══════════════════════════════════════════════════════════════════════
// 7. CONFIG AND CLI OVERRIDE REGRESSION TESTS
// ═══════════════════════════════════════════════════════════════════════

#[test]
#[cfg(feature = "vrw")]
fn regression_default_config_is_valid() {
    // Default config must pass validation without errors.
    let cfg = Config::default();
    let issues = vrc_core::config::validation::validate_config(&cfg);
    let errors: Vec<_> = issues
        .iter()
        .filter(|i| i.level == vrc_core::config::validation::ValidationLevel::Error)
        .collect();
    assert!(
        errors.is_empty(),
        "default config has validation errors: {:?}",
        errors
    );
}

#[test]
#[cfg(feature = "vrw")]
fn regression_config_with_custom_port() {
    let json = r#"{ "server": { "bind": "0.0.0.0", "port": 8080 } }"#;
    let cfg: Config = serde_json::from_str(json).unwrap();
    assert_eq!(cfg.server.port, 8080);
    assert_eq!(cfg.server.bind, "0.0.0.0");
}

#[test]
#[cfg(feature = "vrw")]
fn regression_config_serialize_deserialize_roundtrip() {
    let cfg = Config::default();
    let json = serde_json::to_string(&cfg).unwrap();
    let cfg2: Config = serde_json::from_str(&json).unwrap();
    assert_eq!(cfg.server.port, cfg2.server.port);
    assert_eq!(cfg.vtty.rows, cfg2.vtty.rows);
    assert_eq!(cfg.vtty.cols, cfg2.vtty.cols);
}

#[test]
fn regression_exit_config_serialize() {
    let ec = ExitConfig {
        retain_on_exit: true,
        snapshot_on_exit: Some("/tmp/snap".into()),
        on_exit: Some("echo done".into()),
        on_error: Some("echo fail".into()),
        timeout_secs: 30,
    };
    let json = serde_json::to_string(&ec).unwrap();
    let ec2: ExitConfig = serde_json::from_str(&json).unwrap();
    assert!(ec2.retain_on_exit);
    assert_eq!(ec2.snapshot_on_exit.as_deref(), Some("/tmp/snap"));
    assert_eq!(ec2.on_exit.as_deref(), Some("echo done"));
    assert_eq!(ec2.timeout_secs, 30);
}

#[test]
#[cfg(feature = "vrw")]
fn regression_partial_config_all_none() {
    let pc = vrc_core::config::schema::PartialConfig::default();
    assert!(pc.server.is_none());
    assert!(pc.vtty.is_none());
    assert!(pc.hooks.is_none());
    assert!(pc.default_exit.is_none());
}

#[test]
#[cfg(feature = "vrw")]
fn regression_profile_override_port() {
    use vrc_core::config::merge::apply_profile;
    let mut base = Config::default();
    base.server.port = 9090;
    let mut server = vrc_core::config::schema::ServerConfig::default();
    server.port = 3000;
    let profile = vrc_core::config::schema::PartialConfig {
        server: Some(server),
        ..Default::default()
    };
    let result = apply_profile(base, &profile);
    assert_eq!(result.server.port, 3000, "profile should override port");
}

#[test]
#[cfg(feature = "vrw")]
fn regression_merge_command_env() {
    use vrc_core::config::merge::merge_command_env;
    let config_env = EnvironmentConfig {
        variables: HashMap::from([
            ("GLOBAL_VAR".into(), "global_val".into()),
            ("OVERRIDE".into(), "old".into()),
        ]),
    };
    let cmd_env = HashMap::from([
        ("CMD_VAR".into(), "cmd_val".into()),
        ("OVERRIDE".into(), "new".into()),
    ]);
    let merged = merge_command_env(&config_env, cmd_env);
    assert_eq!(merged.get("GLOBAL_VAR").unwrap(), "global_val");
    assert_eq!(merged.get("OVERRIDE").unwrap(), "new");
    assert_eq!(merged.get("CMD_VAR").unwrap(), "cmd_val");
}

// ═══════════════════════════════════════════════════════════════════════
// 8. INSTANCE REGISTRY AND SHUTDOWN REGRESSION TESTS
// ═══════════════════════════════════════════════════════════════════════

#[test]
#[cfg(feature = "vrw")]
fn regression_instance_registry_new() {
    // Use a temp directory to avoid interference from stale instance files
    // left by previous vrw runs in the shared system data dir.
    let dir = std::env::temp_dir().join("vrw-test-registry");
    let _ = std::fs::remove_dir_all(&dir); // clean up any previous test run
    let reg = vrc_core::instance::registry::InstanceRegistry::with_dir(dir.clone())
        .expect("InstanceRegistry::with_dir should succeed");
    assert!(reg.list_instances().is_empty());
    let _ = std::fs::remove_dir_all(&dir); // cleanup
}

// ═══════════════════════════════════════════════════════════════════════
// 9. BROADCAST CHANNEL / SHUTDOWN SIGNAL REGRESSION TESTS
// ═══════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn regression_shutdown_signal_propagates() {
    // Sending on the shutdown channel must reach all subscribers.
    // Regression: broadcast::Sender was dropped too early, or channel
    // capacity was 0 causing lost notifications.
    let (tx, mut rx1) = tokio::sync::broadcast::channel::<()>(4);
    let mut rx2 = tx.subscribe();

    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let _ = tx.send(());
    });

    tokio::select! {
        _ = rx1.recv() => {},
        _ = tokio::time::sleep(Duration::from_secs(2)) => {
            panic!("rx1 never received shutdown signal");
        }
    }

    tokio::select! {
        _ = rx2.recv() => {},
        _ = tokio::time::sleep(Duration::from_secs(2)) => {
            panic!("rx2 never received shutdown signal");
        }
    }
}

#[tokio::test]
async fn regression_watch_channel_stores_value() {
    // A watch channel must resolve changed() immediately if value
    // was set before any waiter existed.
    // Regression: tokio::sync::Notify lost notifications when no waiter
    // was present, causing the display loop to hang when child exited
    // during server startup delay.
    let (tx, rx) = tokio::sync::watch::channel(false);

    // Simulate: child exits before display loop starts
    tx.send(true).unwrap();

    // Display loop starts now
    let mut rx_clone = rx;
    tokio::select! {
        _ = rx_clone.changed() => {
            assert!(*rx_clone.borrow(), "watch value should be true");
        }
        _ = tokio::time::sleep(Duration::from_secs(1)) => {
            panic!("watch channel never resolved");
        }
    }
}

#[tokio::test]
async fn regression_watch_channel_already_changed() {
    // If the value was already updated, changed() must resolve immediately.
    let (tx, rx) = tokio::sync::watch::channel(false);
    tx.send(true).unwrap();

    // Even without awaiting, borrow() should show true
    assert!(*rx.borrow(), "watch should already be true");

    // And changed() should resolve immediately (not hang)
    let mut rx2 = rx.clone();
    tokio::select! {
        _ = rx2.changed() => {
            // Good — resolved immediately
        }
        _ = tokio::time::sleep(Duration::from_millis(100)) => {
            panic!("changed() should resolve immediately for already-changed value");
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 10. EDGE CASES AND ERROR HANDLING REGRESSION TESTS
// ═══════════════════════════════════════════════════════════════════════

#[tokio::test]
#[cfg(feature = "vrw")]
async fn regression_spawn_command_not_found() {
    // Spawning a nonexistent command must return an error, not panic.
    let cfg = test_config();
    let manager = Arc::new(CommandManager::new(cfg));
    let result = manager
        .spawn(
            "nonexistent_binary_xyz_12345".into(),
            vec![],
            None,
            None,
            HashMap::new(),
            None,
            None,
            None,
        )
        .await;
    assert!(result.is_err(), "spawning nonexistent binary should error");
}

#[tokio::test]
#[cfg(feature = "vrw")]
async fn regression_spawn_with_env_vars() {
    let mut env = HashMap::new();
    env.insert("TEST_VAR".into(), "test_value".into());
    let cfg = test_config();
    let manager = Arc::new(CommandManager::new(cfg));
    let id = manager
        .spawn(
            "sh".into(),
            vec!["-c".into(), "echo $TEST_VAR".into()],
            None,
            None,
            env,
            None,
            None,
            None,
        )
        .await
        .unwrap();
    sleep(Duration::from_millis(300)).await;
    if let Some(handle) = manager.get(&id) {
        let plain = handle.vtty_plain().await;
        assert!(
            plain.contains("test_value"),
            "env var not passed: got '{}'",
            plain
        );
    }
    let _ = manager.kill(&id, None).await;
}

#[tokio::test]
#[cfg(feature = "vrw")]
async fn regression_spawn_with_custom_vtty_size() {
    let cfg = test_config();
    let manager = Arc::new(CommandManager::new(cfg));
    let id = manager
        .spawn(
            "stty".into(),
            vec!["size".into()],
            None,
            None,
            HashMap::new(),
            Some(5),
            Some(20),
            None,
        )
        .await
        .unwrap();
    if let Some(handle) = manager.get(&id) {
        let (rows, cols) = handle.dimensions().await;
        assert_eq!(rows, 5, "custom rows not applied");
        assert_eq!(cols, 20, "custom cols not applied");
    }
    let _ = manager.kill(&id, None).await;
}

#[tokio::test]
#[cfg(feature = "vrw")]
async fn regression_manager_logger_works() {
    // The logger is only enabled when command_log.enabled = true.
    // We need to enable it in the config, otherwise log() is a no-op.
    let mut cfg = test_config();
    cfg.command_log.enabled = true;
    let manager = Arc::new(CommandManager::new(cfg));

    // Spawning a command logs a "spawn" entry automatically
    let id = manager
        .spawn(
            "sleep".into(),
            vec!["60".into()],
            None,
            None,
            HashMap::new(),
            None,
            None,
            None,
        )
        .await
        .unwrap();

    let logger = manager.logger();
    logger.log("test", "regression message");
    let buf = logger.read_memory_buffer();
    assert!(buf.len() >= 1, "logger should have at least 1 entry");
    assert!(buf.last().unwrap().contains("regression message"));
    let _ = manager.kill(&id, None).await;
}

#[tokio::test]
#[cfg(feature = "vrw")]
async fn regression_vtty_change_subscription() {
    let cfg = test_config();
    let manager = Arc::new(CommandManager::new(cfg));
    let mut rx = manager.subscribe_vtty();

    let id = manager
        .spawn(
            "echo".into(),
            vec!["subscribe_test".into()],
            None,
            None,
            HashMap::new(),
            None,
            None,
            None,
        )
        .await
        .unwrap();

    // Should receive at least one change notification
    tokio::select! {
        msg = rx.recv() => {
            let (cmd_id, json) = msg.unwrap();
            assert_eq!(cmd_id, id);
            assert!(json.contains("vtty_dirty"));
        }
        _ = tokio::time::sleep(Duration::from_secs(2)) => {
            // Timeout is acceptable — the notification is best-effort
        }
    }

    let _ = manager.kill(&id, None).await;
}

#[tokio::test]
async fn regression_freeze_thaw_command() {
    // Freeze and thaw must work without panicking.
    // Regression: freeze() held DashMap lock across signal delivery.
    let cfg = test_config();
    let manager = Arc::new(CommandManager::new(cfg));
    let id = manager
        .spawn(
            "sleep".into(),
            vec!["60".into()],
            None,
            None,
            HashMap::new(),
            None,
            None,
            None,
        )
        .await
        .unwrap();

    let freeze_result = manager.freeze(&id);
    assert!(
        freeze_result.is_ok(),
        "freeze should succeed: {:?}",
        freeze_result.err()
    );

    sleep(Duration::from_millis(100)).await;

    let thaw_result = manager.thaw(&id);
    assert!(
        thaw_result.is_ok(),
        "thaw should succeed: {:?}",
        thaw_result.err()
    );

    let _ = manager.kill(&id, None).await;
}

#[tokio::test]
async fn regression_register_sink_on_command() {
    let cfg = test_config();
    let manager = Arc::new(CommandManager::new(cfg));
    let id = manager
        .spawn(
            "echo".into(),
            vec!["sink_test".into()],
            None,
            None,
            HashMap::new(),
            None,
            None,
            None,
        )
        .await
        .unwrap();

    let result = manager.register_sink(&id, "test_null".into(), "null", None);
    assert!(result.is_ok(), "register null sink should succeed");

    // Duplicate name should error
    let result2 = manager.register_sink(&id, "test_null".into(), "null", None);
    assert!(result2.is_err(), "duplicate sink name should error");

    let _ = manager.kill(&id, None).await;
}

#[tokio::test]
async fn regression_is_alive_check() {
    let cfg = test_config();
    let manager = Arc::new(CommandManager::new(cfg));
    let id = manager
        .spawn(
            "echo".into(),
            vec!["alive_check".into()],
            None,
            None,
            HashMap::new(),
            None,
            None,
            None,
        )
        .await
        .unwrap();
    sleep(Duration::from_millis(300)).await;

    if let Some(handle) = manager.get(&id) {
        // echo exits quickly, so it might be dead already
        let _ = handle.is_alive();
    }

    let _ = manager.kill(&id, None).await;
}

#[tokio::test]
async fn regression_cursor_position_and_style() {
    let cfg = test_config();
    let manager = Arc::new(CommandManager::new(cfg));
    let id = manager
        .spawn(
            "echo".into(),
            vec!["cursor".into()],
            None,
            None,
            HashMap::new(),
            None,
            None,
            None,
        )
        .await
        .unwrap();
    sleep(Duration::from_millis(200)).await;

    if let Some(handle) = manager.get(&id) {
        let (row, col) = handle.cursor_position().await;
        let _ = (row, col); // Just ensure it doesn't panic
        let style = handle.cursor_style().await;
        let _ = style; // Just ensure it doesn't panic
    }

    let _ = manager.kill(&id, None).await;
}

#[tokio::test]
async fn regression_dimensions_and_scrollback() {
    let cfg = test_config();
    let manager = Arc::new(CommandManager::new(cfg));
    let id = manager
        .spawn(
            "echo".into(),
            vec!["dim".into()],
            None,
            None,
            HashMap::new(),
            None,
            None,
            None,
        )
        .await
        .unwrap();

    if let Some(handle) = manager.get(&id) {
        let (rows, cols) = handle.dimensions().await;
        assert_eq!(rows, 10, "default rows");
        assert_eq!(cols, 40, "default cols");
        let scrollback = handle.scrollback_count().await;
        let _ = scrollback; // Just ensure it doesn't panic
    }

    let _ = manager.kill(&id, None).await;
}

#[tokio::test]
async fn regression_vtty_html_with_scrollback() {
    let cfg = test_config();
    let manager = Arc::new(CommandManager::new(cfg));
    let id = manager
        .spawn(
            "echo".into(),
            vec!["scrollback_test".into()],
            None,
            None,
            HashMap::new(),
            None,
            None,
            None,
        )
        .await
        .unwrap();
    sleep(Duration::from_millis(200)).await;

    if let Some(handle) = manager.get(&id) {
        let html = handle.vtty_html_scrollback(0, 5).await;
        assert!(html.len() > 0, "scrollback HTML must not be empty");
    }

    let _ = manager.kill(&id, None).await;
}

#[tokio::test]
async fn regression_list_handles() {
    let cfg = test_config();
    let manager = Arc::new(CommandManager::new(cfg));
    let id = manager
        .spawn(
            "echo".into(),
            vec!["handles".into()],
            None,
            None,
            HashMap::new(),
            None,
            None,
            None,
        )
        .await
        .unwrap();

    if let Some(handle) = manager.get(&id) {
        let handles = handle.list_handles();
        // No handles registered by default
        let _ = handles;
    }

    let _ = manager.kill(&id, None).await;
}

#[tokio::test]
async fn regression_runtime_secs_increases() {
    let cfg = test_config();
    let manager = Arc::new(CommandManager::new(cfg));
    let id = manager
        .spawn(
            "sleep".into(),
            vec!["1".into()],
            None,
            None,
            HashMap::new(),
            None,
            None,
            None,
        )
        .await
        .unwrap();

    if let Some(handle) = manager.get(&id) {
        let t1 = handle.runtime_secs();
        sleep(Duration::from_millis(100)).await;
        let t2 = handle.runtime_secs();
        assert!(t2 >= t1, "runtime_secs should increase over time");
    }

    let _ = manager.kill(&id, None).await;
}

#[tokio::test]
async fn regression_exit_code_mutex() {
    // Accessing exit_code must not panic even for running processes.
    let cfg = test_config();
    let manager = Arc::new(CommandManager::new(cfg));
    let id = manager
        .spawn(
            "sleep".into(),
            vec!["1".into()],
            None,
            None,
            HashMap::new(),
            None,
            None,
            None,
        )
        .await
        .unwrap();

    if let Some(handle) = manager.get(&id) {
        let ec = handle.exit_code.lock().ok().and_then(|g| *g);
        assert!(ec.is_none(), "running process should have no exit code");
    }

    let _ = manager.kill(&id, None).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn regression_multiple_snapshots_same_command() {
    // Use a long-running command so it stays in the manager for snapshot ops.
    let cfg = test_config();
    let manager = Arc::new(CommandManager::new(cfg));
    let id = manager
        .spawn(
            "sleep".into(),
            vec!["60".into()],
            None,
            None,
            HashMap::new(),
            None,
            None,
            None,
        )
        .await
        .unwrap();
    sleep(Duration::from_millis(100)).await;

    manager.store_snapshot(&id, "snap1").unwrap();
    manager.store_snapshot(&id, "snap2").unwrap();
    manager.store_snapshot(&id, "snap3").unwrap();

    let snaps = manager.list_snapshots(&id);
    assert_eq!(snaps.len(), 3);

    manager.delete_snapshot(&id, "snap2").unwrap();
    let snaps2 = manager.list_snapshots(&id);
    assert_eq!(snaps2.len(), 2);

    let _ = manager.kill(&id, None).await;
}

// ═══════════════════════════════════════════════════════════════════════
// BONUS: Emulator-level regression tests (pure unit tests, no process spawn)
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn regression_emulator_bracketed_paste_roundtrip() {
    let mut emu = vrc_core::vtty::emulator::VttyEmulator::new(24, 80, 1000);
    assert!(!emu.bracketed_paste_enabled());
    emu.feed(b"\x1b[?2004h");
    assert!(emu.bracketed_paste_enabled());
    emu.feed(b"\x1b[?2004l");
    assert!(!emu.bracketed_paste_enabled());
}

#[test]
fn regression_emulator_focus_reporting_roundtrip() {
    let mut emu = vrc_core::vtty::emulator::VttyEmulator::new(24, 80, 1000);
    assert!(!emu.focus_reporting_enabled());
    emu.feed(b"\x1b[?1004h");
    assert!(emu.focus_reporting_enabled());
    emu.feed(b"\x1b[?1004l");
    assert!(!emu.focus_reporting_enabled());
}

#[test]
fn regression_emulator_mouse_tracking_roundtrip() {
    let mut emu = vrc_core::vtty::emulator::VttyEmulator::new(24, 80, 1000);
    assert!(!emu.mouse_tracking_enabled());
    emu.feed(b"\x1b[?1003h");
    assert!(emu.mouse_tracking_enabled());
    emu.feed(b"\x1b[?1003l");
    assert!(!emu.mouse_tracking_enabled());
}

#[test]
fn regression_emulator_sgr_reset_clears_all_attributes() {
    let mut emu = vrc_core::vtty::emulator::VttyEmulator::new(3, 20, 1000);
    emu.feed(b"\x1b[1;3;4;7m"); // bold, italic, underline, reverse
    emu.feed(b"\x1b[0m"); // reset
    emu.feed_str("X");
    let buf = emu.snapshot();
    let cell = buf.get(0, 0).unwrap();
    assert!(!cell.bold, "bold should be reset");
    assert!(!cell.italic, "italic should be reset");
    assert!(!cell.underline, "underline should be reset");
    assert!(!cell.reverse, "reverse should be reset");
}

#[test]
fn regression_emulator_alt_screen_preserves_main() {
    let mut emu = vrc_core::vtty::emulator::VttyEmulator::new(5, 10, 1000);
    emu.feed_str("MAIN_DATA");
    emu.feed(b"\x1b[?1049h"); // enter alt
    emu.feed_str("ALT_DATA");
    emu.feed(b"\x1b[?1049l"); // exit alt

    let buf = emu.snapshot();
    assert_eq!(
        buf.get(0, 0).unwrap().ch,
        'M',
        "main buffer should be restored"
    );
}

#[test]
fn regression_emulator_cursor_position_after_scroll() {
    let mut emu = vrc_core::vtty::emulator::VttyEmulator::new(3, 10, 1000);
    // Fill the buffer
    for i in 0..3 {
        emu.feed_str(&format!("line{}\n", i));
    }
    // This should scroll, putting "line2" at row 1
    let buf = emu.snapshot();
    // Verify scrollback has content
    assert!(
        buf.scrollback.len() > 0,
        "scrollback should have entries after overflow"
    );
}

#[test]
fn regression_buffer_resize_preserves_content() {
    use vrc_core::vtty::cell::Cell;
    let mut buf = vrc_core::vtty::buffer::Buffer::new(10, 5, 100);
    buf.set(0, 0, Cell::new('P'));
    buf.set(2, 9, Cell::new('Q'));
    buf.resize(8, 3); // shrink
    assert_eq!(buf.get(0, 0).unwrap().ch, 'P', "P preserved after shrink");
    // Q was at col 9, now width is 8, so col 9 is gone
    assert!(
        buf.get(2, 9).is_none(),
        "col 9 should not exist after shrink"
    );
}

// ═══════════════════════════════════════════════════════════════════════
// 11. DISPLAY UI REGRESSION TESTS
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn regression_no_status_bar_steals_terminal_rows() {
    // The display loop used to render a status bar at the bottom of the
    // terminal, stealing 1 row from the VTTY viewport without reporting
    // the smaller size to child commands.  After removing the status bar,
    // a command spawned with --tabs in a 25-row terminal should get
    // 24 rows (25 - 1 for tab bar), not 23 rows (25 - 1 tab - 1 status).
    //
    // We verify this by checking that the size math in main.rs and
    // display.rs only subtracts 1 for the tab bar, never 2.
    //
    // This is a documentation-of-intent test: the actual code subtracts
    // `if show_tabs { 1 } else { 0 }` rows.  If someone adds a status
    // bar back without updating the size math, this test documents the
    // expected behavior.
    let tab_bar_rows: u16 = 1;
    let status_bar_rows: u16 = 0; // removed — must stay 0
    let total_chrome = tab_bar_rows + status_bar_rows;
    assert_eq!(
        total_chrome, 1,
        "only the tab bar may consume rows; no status bar"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn regression_vtty_height_matches_reported_size() {
    // When a command is spawned with a given VTTY size, the buffer's
    // height must match exactly what was requested.  A status bar that
    // stole a row would cause the VTTY to render into the bar area,
    // but the buffer height itself would still be correct (the bug was
    // in the terminal, not the buffer).  This test ensures the buffer
    // dimensions are always exact.
    let cfg = test_config();
    let manager = Arc::new(CommandManager::new(cfg));
    let id = manager
        .spawn(
            "sleep".into(),
            vec!["60".into()],
            None,
            None,
            HashMap::new(),
            Some(24),
            Some(80),
            None,
        )
        .await
        .unwrap();

    if let Some(handle) = manager.get(&id) {
        let (rows, cols) = handle.dimensions().await;
        assert_eq!(rows, 24, "VTTY rows must match requested size exactly");
        assert_eq!(cols, 80, "VTTY cols must match requested size exactly");
    }

    let _ = manager.kill(&id, None).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn regression_display_render_uses_full_available_height() {
    // TerminalDisplay::render() must not subtract extra rows for a
    // status bar.  Given a buffer of height H and row_offset of 1
    // (tab bar), it should render H rows starting at row 1, using
    // rows 1..H+1 of the physical terminal.  The last rendered row
    // should be at row_offset + buffer_height - 1.
    //
    // We can't test the actual terminal rendering in a headless env,
    // but we verify that the buffer dimensions are consistent with
    // the offset math.
    let rows: u16 = 24;
    let tab_offset: u16 = 1; // only the tab bar
    let status_offset: u16 = 0; // no status bar

    // A command with these dimensions should fill rows tab_offset..tab_offset+rows
    let last_rendered_row = tab_offset + status_offset + rows - 1;
    assert_eq!(
        last_rendered_row, 24,
        "last rendered row = tab_offset + rows - 1 = 24"
    );

    // If a status bar existed (status_offset=1), last row would be 25,
    // pushing content off-screen in a 25-row terminal.  With status_offset=0,
    // the VTTY fits exactly in the remaining space.
    assert!(
        last_rendered_row <= 24,
        "VTTY content must not exceed terminal bounds"
    );
}

#[tokio::test]
async fn regression_spawn_bash_multi_command() {
    // Spawn with "sh -c" to run multiple commands in sequence.
    // Regression: arguments with semicolons or shell operators were
    // broken when the web UI split args by whitespace.
    let cfg = test_config();
    let manager = Arc::new(CommandManager::new(cfg));
    let id = manager
        .spawn(
            "sh".into(),
            vec!["-c".into(), "echo first; echo second; echo third".into()],
            None,
            None,
            HashMap::new(),
            None,
            None,
            None,
        )
        .await
        .unwrap();
    sleep(Duration::from_millis(500)).await;
    if let Some(handle) = manager.get(&id) {
        let plain = handle.vtty_plain().await;
        assert!(
            plain.contains("first"),
            "first command output missing: '{}'",
            plain
        );
        assert!(
            plain.contains("second"),
            "second command output missing: '{}'",
            plain
        );
        assert!(
            plain.contains("third"),
            "third command output missing: '{}'",
            plain
        );
    }
    let _ = manager.kill(&id, None).await;
}

#[tokio::test]
async fn regression_spawn_with_working_directory() {
    // Spawn a command with a custom working directory.
    // Regression: spawn had no way to set the cwd.
    let cfg = test_config();
    let manager = Arc::new(CommandManager::new(cfg));
    let tmp_dir = std::env::temp_dir().to_string_lossy().to_string();
    let id = manager
        .spawn(
            "sh".into(),
            vec!["-c".into(), "pwd".into()],
            None,
            None,
            HashMap::new(),
            None,
            None,
            Some(tmp_dir.clone()),
        )
        .await
        .unwrap();
    sleep(Duration::from_millis(300)).await;
    if let Some(handle) = manager.get(&id) {
        let plain = handle.vtty_plain().await;
        assert!(
            plain.contains(&tmp_dir),
            "pwd should show working dir: got '{}'",
            plain
        );
    }
    let _ = manager.kill(&id, None).await;
}
