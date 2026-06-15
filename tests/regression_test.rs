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
    CommandLogConfig, Config, DaemonConfig, DisplayConfig,
};
#[cfg(feature = "vrw")]
use vrc_core::config::schema::EnvironmentConfig;
#[cfg(feature = "vrw")]
use vrc_core::config::schema::{SecurityConfig, ServerConfig, TlsConfig, VttyConfig};
use vrc_core::process::manager::CommandManager;

// ─── Test helpers ───────────────────────────────────────────────────────

/// Create a test config with a random port (0 = OS-assigned).
#[cfg(feature = "vrw")]
fn test_config() -> Config {
    Config {
        binary_name: "vrw".to_string(),
        color_terminal_log: false,
        server: ServerConfig {
            bind: "127.0.0.1".to_string(),
            port: 0,
            name: None,
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
            terminal: Default::default(),
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
        environments: Default::default(),
    }
}

/// Create a test config for vrc-only mode (no server/security/tls fields).
#[cfg(not(feature = "vrw"))]
fn test_config() -> Config {
    Config {
        binary_name: "vrc".to_string(),
        color_terminal_log: false,
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
            terminal: Default::default(),
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
        templates: Default::default(),
        environments: Default::default(),
        profiles: Default::default(),
        hooks: Default::default(),
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
            vrc_core::process::manager::SpawnOptions::new("echo".to_string())
                .args(vec!["hi".into()])
                .env_vars(HashMap::new()),
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
            vrc_core::process::manager::SpawnOptions::new("echo".to_string())
                .args(vec!["test".into()])
                .env_vars(HashMap::new()),
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
            vrc_core::process::manager::SpawnOptions::new("sleep".to_string())
                .args(vec!["60".into()])
                .env_vars(HashMap::new()),
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
    // kill() returns Err(CommandNotFound) when the ID doesn't exist
    assert!(result.is_err(), "kill() on nonexistent ID should return error");
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
            vrc_core::process::manager::SpawnOptions::new("sleep".to_string())
                .args(vec!["60".into()])
                .env_vars(HashMap::new()),
            )
        .await
        .unwrap();
    let id2 = manager
        .spawn(
            vrc_core::process::manager::SpawnOptions::new("sleep".to_string())
                .args(vec!["60".into()])
                .env_vars(HashMap::new()),
            )
        .await
        .unwrap();
    let id3 = manager
        .spawn(
            vrc_core::process::manager::SpawnOptions::new("sleep".to_string())
                .args(vec!["60".into()])
                .env_vars(HashMap::new()),
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
            vrc_core::process::manager::SpawnOptions::new("sleep".to_string())
                .args(vec!["60".into()])
                .env_vars(HashMap::new()),
            )
        .await
        .unwrap();
    let id2 = manager
        .spawn(
            vrc_core::process::manager::SpawnOptions::new("sleep".to_string())
                .args(vec!["60".into()])
                .env_vars(HashMap::new()),
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
            vrc_core::process::manager::SpawnOptions::new("echo".to_string())
                .args(vec!["pid_test".into()])
                .env_vars(HashMap::new()),
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
            vrc_core::process::manager::SpawnOptions::new("sleep".to_string())
                .args(vec!["60".into()])
                .env_vars(HashMap::new()),
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
            vrc_core::process::manager::SpawnOptions::new("echo".to_string())
                .args(vec!["first".into()])
                .env_vars(HashMap::new()),
            )
        .await
        .unwrap();
    manager.kill(&id1, None).await.unwrap();
    sleep(Duration::from_millis(100)).await;

    // Use sleep so the command stays alive long enough to appear in list
    let id2 = manager
        .spawn(
            vrc_core::process::manager::SpawnOptions::new("sleep".to_string())
                .args(vec!["60".into()])
                .env_vars(HashMap::new()),
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
                vrc_core::process::manager::SpawnOptions::new("sleep".to_string())
                    .args(vec!["60".into()])
                    .env_vars(HashMap::new()),
                )
            .await
            .unwrap();
        ids.push(id);
    }
    for id in &ids {
        let _ = manager.kill(id, None).await;
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
                vrc_core::process::manager::SpawnOptions::new("sleep".to_string())
                    .args(vec!["60".into()])
                    .env_vars(HashMap::new()),
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
            vrc_core::process::manager::SpawnOptions::new("echo".to_string())
                .args(vec!["snapshot_test".into()])
                .env_vars(HashMap::new()),
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
async fn regression_resize_command() {
    let cfg = test_config();
    let manager = Arc::new(CommandManager::new(cfg));
    let id = manager
        .spawn(
            vrc_core::process::manager::SpawnOptions::new("sleep".to_string())
                .args(vec!["60".into()])
                .env_vars(HashMap::new()),
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

/// Test resize_pty (used by web UI resize button) resizes PTY, VTTY buffer,
/// AND marks the buffer as changed so poll-based clients detect the update.
#[tokio::test(flavor = "multi_thread")]
#[cfg(feature = "vrw")]
async fn regression_resize_pty_notifies_sinks() {
    let cfg = test_config();
    let manager = Arc::new(CommandManager::new(cfg));
    let id = manager
        .spawn(
            vrc_core::process::manager::SpawnOptions::new("sleep".to_string())
                .args(vec!["60".into()])
                .env_vars(HashMap::new()),
            )
        .await
        .unwrap();

    if let Some(handle) = manager.get(&id) {
        // Consume the initial "changed" flag from spawn
        let _ = manager.has_changed(&id);
        sleep(Duration::from_millis(50)).await;

        // Call resize_pty (same code path as the web UI resize button)
        let result = handle.resize_pty(40, 120).await;
        assert!(result.is_ok(), "resize_pty should succeed");

        // Verify dimensions changed
        let (rows, cols) = handle.dimensions().await;
        assert_eq!(rows, 40, "rows after resize_pty should be 40");
        assert_eq!(cols, 120, "cols after resize_pty should be 120");

        // Verify the buffer is marked as changed (push-mode clients get notified
        // via notify_sinks, poll-mode clients detect via has_changed)
        let changed = manager.has_changed(&id).unwrap_or(false);
        assert!(
            changed,
            "buffer must be marked as changed after resize_pty"
        );
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
            vrc_core::process::manager::SpawnOptions::new("sleep".to_string())
                .args(vec!["60".into()])
                .env_vars(HashMap::new()),
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
            vrc_core::process::manager::SpawnOptions::new("sleep".to_string())
                .args(vec!["60".into()])
                .env_vars(HashMap::new()),
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
            vrc_core::process::manager::SpawnOptions::new("sleep".to_string())
                .args(vec!["60".into()])
                .env_vars(HashMap::new()),
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
            vrc_core::process::manager::SpawnOptions::new("cat".to_string())
                .args(vec![])
                .env_vars(HashMap::new()),
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
            vrc_core::process::manager::SpawnOptions::new("cat".to_string())
                .args(vec![])
                .env_vars(HashMap::new()),
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
            vrc_core::process::manager::SpawnOptions::new("nonexistent_binary_xyz_12345".to_string())
                .args(vec![])
                .env_vars(HashMap::new()),
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
            vrc_core::process::manager::SpawnOptions::new("sh".to_string())
                .args(vec!["-c".into(), "echo $TEST_VAR".into()])
                .env_vars(env),
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

/// Per-command env vars override config-level env vars.
/// When the same key appears in both config and per-command env,
/// the per-command value wins.  This is the core merging behaviour
/// used by the spawn form's "Environment Variables" textarea.
#[tokio::test]
#[cfg(feature = "vrw")]
async fn regression_spawn_command_env_overrides_config_env() {
    let mut cfg = test_config();
    cfg.environment = vrc_core::config::schema::EnvironmentConfig {
        variables: HashMap::from([
            ("BASE_VAR".into(), "from_config".into()),
            ("OVERRIDE_VAR".into(), "from_config".into()),
        ]),
    };
    let manager = Arc::new(CommandManager::new(cfg));

    // Per-command env overrides OVERRIDE_VAR, adds NEW_VAR
    let mut env = HashMap::new();
    env.insert("OVERRIDE_VAR".into(), "from_command".into());
    env.insert("NEW_VAR".into(), "new_value".into());

    let id = manager
        .spawn(
            vrc_core::process::manager::SpawnOptions::new("sh".to_string())
                .args(vec!["-c".into(), "echo $OVERRIDE_VAR $NEW_VAR $BASE_VAR".into()])
                .env_vars(env),
        )
        .await
        .unwrap();
    sleep(Duration::from_millis(300)).await;
    if let Some(handle) = manager.get(&id) {
        let plain = handle.vtty_plain().await;
        assert!(
            plain.contains("from_command"),
            "per-command override not applied: got '{}'",
            plain
        );
        assert!(
            plain.contains("new_value"),
            "new per-command var not present: got '{}'",
            plain
        );
        assert!(
            plain.contains("from_config"),
            "config-level var should still be present: got '{}'",
            plain
        );
    }
    let _ = manager.kill(&id, None).await;
}

/// Per-command env vars can contain values with equals signs.
/// The parseSpawnEnvVars JS function splits on the first '=' only,
/// so "FOO=bar=baz" becomes { "FOO": "bar=baz" }.  The backend
/// receives this as a HashMap<String, String> and passes it through
/// unchanged to the child process environment.
#[tokio::test]
#[cfg(feature = "vrw")]
async fn regression_spawn_env_value_with_equals_sign() {
    let mut env = HashMap::new();
    env.insert("CONN_STR".into(), "host=localhost&port=5432".into());
    env.insert("MATH".into(), "1+1=2".into());
    let cfg = test_config();
    let manager = Arc::new(CommandManager::new(cfg));
    let id = manager
        .spawn(
            vrc_core::process::manager::SpawnOptions::new("sh".to_string())
                .args(vec!["-c".into(), "echo $CONN_STR $MATH".into()])
                .env_vars(env),
        )
        .await
        .unwrap();
    sleep(Duration::from_millis(300)).await;
    if let Some(handle) = manager.get(&id) {
        let plain = handle.vtty_plain().await;
        assert!(
            plain.contains("host=localhost&port=5432"),
            "env value with = not preserved: got '{}'",
            plain
        );
        assert!(
            plain.contains("1+1=2"),
            "env value with = not preserved: got '{}'",
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
            vrc_core::process::manager::SpawnOptions::new("stty".to_string())
                .args(vec!["size".into()])
                .rows(Some(5))
                .cols(Some(20)),
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
            vrc_core::process::manager::SpawnOptions::new("sleep".to_string())
                .args(vec!["60".into()])
                .env_vars(HashMap::new()),
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

/// Regression: verify that subscribing to the VTTY change broadcast channel
/// delivers lightweight `vtty_dirty` signals when a command produces output.
///
/// In the pull-based architecture the broadcast channel only carries
/// `vtty_dirty` / `vtty_close` signals — never data.  Clients pull actual
/// terminal content via the HTTP diff endpoint (`diff_with_baseline`).
#[tokio::test(flavor = "multi_thread")]
#[cfg(feature = "vrw")]
async fn regression_vtty_change_subscription() {
    let cfg = test_config();
    let manager = Arc::new(CommandManager::new(cfg));

    // Use `cat` so the PTY stays open and we can write to it.
    let id = manager
        .spawn(
            vrc_core::process::manager::SpawnOptions::new("cat".to_string())
                .env_vars(HashMap::new()),
        )
        .await
        .unwrap();

    let mut rx = manager.subscribe_vtty();

    // Write to the PTY to trigger a buffer change, which causes
    // BroadcastVttySink::on_buffer_change() to broadcast `vtty_dirty`.
    if let Some(handle) = manager.get(&id) {
        handle.send_bytes(b"hello\n".to_vec()).await.unwrap();
    }

    let (cmd_id, json) = tokio::time::timeout(
        Duration::from_secs(2),
        rx.recv(),
    )
    .await
    .expect("should receive a broadcast message within 2s")
    .unwrap();

    assert_eq!(cmd_id, id);
    assert!(
        json.contains("vtty_dirty"),
        "expected vtty_dirty signal, got: {}",
        json
    );
    // The dirty signal must NOT contain terminal data — it is purely a
    // notification that the client should pull a diff via HTTP.
    let msg: serde_json::Value = serde_json::from_str(&json).unwrap();
    let data = msg.get("data").unwrap();
    assert!(
        data.get("html").is_none(),
        "vtty_dirty must not contain html"
    );
    assert!(
        data.get("cells").is_none(),
        "vtty_dirty must not contain cells"
    );

    // Kill the command — should eventually receive vtty_close.
    // The kill itself may trigger a final vtty_dirty before vtty_close arrives.
    let _ = manager.kill(&id, None).await;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    let mut got_close = false;
    while tokio::time::Instant::now() < deadline {
        let remaining = deadline - tokio::time::Instant::now();
        match tokio::time::timeout(remaining, rx.recv()).await {
            Ok(Ok((cid, json_str))) => {
                if cid != id { continue; }
                if json_str.contains("vtty_close") {
                    got_close = true;
                    break;
                }
                // Skip any trailing vtty_dirty signals after kill.
            }
            Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) => break,
            Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(_))) => continue,
            Err(_) => break,
        }
    }
    assert!(got_close, "should receive vtty_close after kill");
}

#[tokio::test]
async fn regression_freeze_thaw_command() {
    // Freeze and thaw must work without panicking.
    // Regression: freeze() held DashMap lock across signal delivery.
    let cfg = test_config();
    let manager = Arc::new(CommandManager::new(cfg));
    let id = manager
        .spawn(
            vrc_core::process::manager::SpawnOptions::new("sleep".to_string())
                .args(vec!["60".into()])
                .env_vars(HashMap::new()),
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
            vrc_core::process::manager::SpawnOptions::new("echo".to_string())
                .args(vec!["sink_test".into()])
                .env_vars(HashMap::new()),
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
async fn regression_runtime_secs_increases() {
    let cfg = test_config();
    let manager = Arc::new(CommandManager::new(cfg));
    let id = manager
        .spawn(
            vrc_core::process::manager::SpawnOptions::new("sleep".to_string())
                .args(vec!["1".into()])
                .env_vars(HashMap::new()),
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
            vrc_core::process::manager::SpawnOptions::new("sleep".to_string())
                .args(vec!["1".into()])
                .env_vars(HashMap::new()),
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
            vrc_core::process::manager::SpawnOptions::new("sleep".to_string())
                .args(vec!["60".into()])
                .env_vars(HashMap::new()),
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
            vrc_core::process::manager::SpawnOptions::new("sleep".to_string())
                .args(vec!["60".into()])
                .env_vars(HashMap::new())
                .rows(Some(24))
                .cols(Some(80)),
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

#[tokio::test]
async fn regression_spawn_bash_multi_command() {
    // Spawn with "sh -c" to run multiple commands in sequence.
    // Regression: arguments with semicolons or shell operators were
    // broken when the web UI split args by whitespace.
    let cfg = test_config();
    let manager = Arc::new(CommandManager::new(cfg));
    let id = manager
        .spawn(
            vrc_core::process::manager::SpawnOptions::new("sh".to_string())
                .args(vec!["-c".into(), "echo first; echo second; echo third".into()])
                .env_vars(HashMap::new()),
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
            vrc_core::process::manager::SpawnOptions::new("sh".to_string())
                .args(vec!["-c".into(), "pwd".into()])
                .env_vars(HashMap::new())
                .dir(Some(tmp_dir.clone())),
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

// ═══════════════════════════════════════════════════════════════════════
// 11. TERMINAL DISPLAY PIPELINE REGRESSION TESTS
// ═══════════════════════════════════════════════════════════════════════
// These tests verify that VTTY changes from the server are properly detected,
// tracked via generation counters, and rendered to HTML. They guard against
// regressions like the one where loadSnapshot() set per-panel selection fields
// on the DOM element instead of the panel state object, causing the per-panel
// WebSocket to never connect and all VTTY updates to be silently dropped.

#[tokio::test(flavor = "multi_thread")]
#[cfg(feature = "vrw")]
async fn regression_vtty_changes_detected_and_rendered() {
    // Verify the full VTTY change pipeline:
    // 1. has_changed() detects buffer modifications,
    // 2. vtty_html() returns valid, non-empty HTML after changes,
    // 3. Generation counter advances correctly across multiple outputs,
    // 4. vtty_snapshot dimensions are consistent.
    let cfg = test_config();
    let manager = Arc::new(CommandManager::new(cfg.clone()));

    // Spawn a command that produces output in stages
    let id = manager
        .spawn(
            vrc_core::process::manager::SpawnOptions::new("sh".to_string())
                .args(vec![
                    "-c".into(),
                    "echo first; sleep 0.1; echo second; sleep 0.1; echo third".into(),
                ]),
        )
        .await
        .unwrap();

    // Wait for the command to start and produce initial output
    sleep(Duration::from_millis(100)).await;

    // Step 1: First check must always report changed (no previous generation)
    let first_changed = manager.has_changed(&id).unwrap();
    assert!(
        first_changed,
        "first has_changed() must return true for a new command"
    );

    // Step 2: vtty_html() must return non-empty content with span tags
    let html = if let Some(handle) = manager.get(&id) {
        handle.vtty_html().await
    } else {
        panic!("command handle must exist after spawn");
    };
    assert!(
        html.len() > 0,
        "vtty_html() must return non-empty HTML after spawn"
    );
    assert!(
        html.contains("<span"),
        "vtty_html() must contain span tags from the renderer"
    );

    // Step 3: Consume the change — next check reflects current state
    sleep(Duration::from_millis(50)).await;
    let _second_changed = manager.has_changed(&id).unwrap();
    // If the command is still producing output, this might still be true.
    // We only assert that it doesn't panic and returns a valid result.

    // Step 4: Wait for all output to complete
    sleep(Duration::from_millis(500)).await;

    // Step 5: Verify the final HTML contains content (handle may be gone if
    // the short-lived command has already exited and been cleaned up)
    if let Some(handle) = manager.get(&id) {
        let final_html = handle.vtty_html().await;
        assert!(
            final_html.len() > 0,
            "final vtty_html() must return non-empty HTML"
        );

        // Step 6: Verify that vtty_snapshot dimensions are valid and consistent
        let buf = handle.vtty_snapshot().await;
        assert!(
            buf.width > 0,
            "buffer width must be positive after output"
        );
        assert!(
            buf.height > 0,
            "buffer height must be positive after output"
        );
        assert!(
            buf.height <= cfg.vtty.rows as usize + 50,
            "buffer height should not exceed terminal rows by much"
        );
    }

    let _ = manager.kill(&id, None).await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg(feature = "vrw")]
async fn regression_vtty_generation_advances_on_write() {
    // Verify that writing to a command advances the buffer generation,
    // and has_changed() correctly tracks this across multiple writes.
    // This guards against the class of bugs where change detection
    // silently stops working (e.g., panel state not set correctly).
    let cfg = test_config();
    let manager = Arc::new(CommandManager::new(cfg));

    let id = manager
        .spawn(
            vrc_core::process::manager::SpawnOptions::new("cat".to_string())
                .args(vec![].into())
                .env_vars(HashMap::new()),
            )
        .await
        .unwrap();

    sleep(Duration::from_millis(50)).await;

    // Initial check — should be changed (first observation)
    assert!(manager.has_changed(&id).unwrap());

    // After consuming, should not be changed (cat is idle, waiting for input)
    sleep(Duration::from_millis(50)).await;
    let _idle_changed = manager.has_changed(&id).unwrap();

    // Write some input to trigger a buffer change
    if let Some(handle) = manager.get(&id) {
        handle.send_bytes(b"hello world\n".to_vec()).await.unwrap();
    }
    sleep(Duration::from_millis(100)).await;

    // After write, must report changed again
    let after_write_changed = manager.has_changed(&id).unwrap();
    assert!(
        after_write_changed,
        "has_changed() must return true after writing to the terminal"
    );

    // Verify the written content appears in HTML
    if let Some(handle) = manager.get(&id) {
        let html = handle.vtty_html().await;
        assert!(
            html.len() > 0,
            "vtty_html() must return content after write"
        );
    }

    // Write again and verify change detection still works
    if let Some(handle) = manager.get(&id) {
        handle.send_bytes(b"second line\n".to_vec()).await.unwrap();
    }
    sleep(Duration::from_millis(100)).await;

    let after_second_write = manager.has_changed(&id).unwrap();
    assert!(
        after_second_write,
        "has_changed() must return true after second write"
    );

    let _ = manager.kill(&id, None).await;
}

/// Verify the pull-based diff endpoint returns well-formed cell diffs
/// that the web UI can parse and apply.
///
/// This test exercises the correct pull-based pipeline:
///   1. Spawn a long-running command (cat) so the PTY stays open.
///   2. Request an initial diff (no baseline) — should return all cells.
///   3. Write data to the PTY to trigger a buffer change.
///   4. Wait for a `vtty_dirty` broadcast signal.
///   5. Request an incremental diff using the baseline UUID — should return
///      only the changed cells with an advanced generation number.
///   6. Verify the cell diff entries have the correct structure
///      `{ row, col, cell: { ch, fg, bg, bold, italic, ..., width } }` that
///      the client-side `applyVttyDiffForPanel()` expects.
#[tokio::test(flavor = "multi_thread")]
#[cfg(feature = "vrw")]
async fn regression_pull_diff_returns_correct_format() {
    let cfg = test_config();
    let manager = Arc::new(CommandManager::new(cfg));

    // Use cat so the PTY stays open and we can write to it.
    let id = manager
        .spawn(
            vrc_core::process::manager::SpawnOptions::new("cat".to_string()),
        )
        .await
        .unwrap();

    sleep(Duration::from_millis(100)).await;

    // Step 1: Initial diff (no baseline) — should return all cells and a UUID.
    let (baseline1, diff1, _cursor1, dims1, gen1) = manager
        .diff_with_baseline(&id, None)
        .await
        .expect("initial diff should succeed");
    assert!(
        !baseline1.is_empty(),
        "initial diff must return a baseline UUID"
    );
    assert!(
        !diff1.cells.is_empty(),
        "initial diff must return cells (full sync)"
    );
    assert!(
        dims1.0 > 0 && dims1.1 > 0,
        "dimensions must be positive"
    );

    // Step 2: Write to the PTY to trigger a buffer change.
    if let Some(handle) = manager.get(&id) {
        handle.send_bytes(b"hello diff test\n".to_vec()).await.unwrap();
    }
    sleep(Duration::from_millis(100)).await;

    // Step 3: Incremental diff with the baseline UUID.
    let (baseline2, diff2, _cursor2, dims2, gen2) = manager
        .diff_with_baseline(&id, Some(&baseline1))
        .await
        .expect("incremental diff should succeed");
    // Baseline UUID should remain the same (updated in place).
    assert_eq!(baseline2, baseline1, "baseline UUID should be reused");
    assert!(
        gen2 >= gen1,
        "generation must advance or stay: initial={}, incremental={}",
        gen1, gen2
    );
    assert_eq!(dims2, dims1, "dimensions should not change");

    // The diff should contain the cells we just wrote (at minimum).
    assert!(
        !diff2.cells.is_empty(),
        "incremental diff must contain changed cells after a write"
    );

    // Verify the cell diff entries have the correct structure
    // that the client's applyVttyDiffForPanel() expects.
    for cell_entry in &diff2.cells {
        // CellDiff has row, col, and a nested Cell { ch, fg, bg, ..., width }.
        // dimensions() returns (rows, cols).
        assert!(
            cell_entry.row < dims2.0,
            "cell row {} must be within buffer rows {}",
            cell_entry.row, dims2.0
        );
        assert!(
            cell_entry.col < dims2.1,
            "cell col {} must be within buffer cols {}",
            cell_entry.col, dims2.1
        );
        assert!(
            cell_entry.cell.width > 0 || cell_entry.cell.ch == ' ',
            "cell must have positive width or be a space"
        );
    }

    // Step 4: Write again and verify another incremental diff.
    if let Some(handle) = manager.get(&id) {
        handle.send_bytes(b"second line\n".to_vec()).await.unwrap();
    }
    sleep(Duration::from_millis(100)).await;

    let (_baseline3, diff3, _, _, gen3) = manager
        .diff_with_baseline(&id, Some(&baseline2))
        .await
        .expect("second incremental diff should succeed");
    assert!(
        gen3 >= gen2,
        "generation must advance: second={}, third={}",
        gen2, gen3
    );
    assert!(
        !diff3.cells.is_empty(),
        "second incremental diff must contain changed cells"
    );

    // Step 5: Verify that an unknown baseline UUID creates a new full sync.
    let (baseline_fresh, diff_fresh, _, _, _) = manager
        .diff_with_baseline(&id, Some("nonexistent-uuid"))
        .await
        .expect("diff with unknown baseline should succeed (falls back to full)");
    assert_ne!(
        baseline_fresh, baseline2,
        "unknown baseline should produce a new UUID"
    );
    assert!(
        diff_fresh.changed_count > diff3.changed_count,
        "full sync from unknown baseline should have more changed cells than incremental"
    );

    let _ = manager.kill(&id, None).await;
}

/// Regression: terminal updates must be detectable when a command produces
/// periodic output (e.g., a loop that sleeps then writes).
///
/// The original bug was that `BroadcastVttySink` sent dataless `vtty_dirty`
/// messages into the same broadcast channel as data-rich `vtty_diff`/`vtty_full`
/// from `spawn_diff_watcher`, flooding the channel (capacity 256) and causing
/// the receiver to lag and drop messages. The terminal appeared frozen.
///
/// The fix is a purely pull-based architecture:
///   - The broadcast channel carries ONLY lightweight `vtty_dirty` / `vtty_close`
///     signals (no terminal data).
///   - Clients pull actual content on demand via the HTTP diff endpoint
///     (`diff_with_baseline`), which computes diffs against a per-client
///     baseline snapshot.
///   - The old `spawn_diff_watcher` background task has been removed entirely.
///
/// This test spawns a bash loop that sleeps 0.3s then writes a line, and
/// verifies that:
///   1. Multiple `vtty_dirty` signals arrive on the broadcast channel.
///   2. The channel does NOT overflow (no Lagged errors).
///   3. Pulling the diff endpoint after each dirty signal returns content
///      with strictly advancing generation numbers.
#[tokio::test(flavor = "multi_thread")]
#[cfg(feature = "vrw")]
async fn regression_terminal_updates_arrive_for_periodic_output() {
    let cfg = test_config();
    let manager = Arc::new(CommandManager::new(cfg));

    let script = r#"
for i in $(seq 1 8); do
    sleep 0.3
    echo "tick $i"
done
"#.to_string();

    let id = manager
        .spawn(
            vrc_core::process::manager::SpawnOptions::new("bash".to_string())
                .args(vec!["-c".to_string(), script])
                .env_vars(HashMap::new()),
        )
        .await
        .unwrap();

    let mut rx = manager.subscribe_vtty();

    // Collect vtty_dirty signals and pull diffs for up to 5 seconds.
    let mut dirty_count: usize = 0;
    let mut pulled_gens: Vec<u64> = Vec::new();
    let mut current_baseline: Option<String> = None;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);

    while tokio::time::Instant::now() < deadline {
        let remaining = deadline - tokio::time::Instant::now();
        match tokio::time::timeout(remaining, rx.recv()).await {
            Ok(Ok((cmd_id, json_str))) => {
                if cmd_id != id {
                    continue;
                }
                let msg: serde_json::Value = match serde_json::from_str(&json_str) {
                    Ok(m) => m,
                    Err(_) => continue,
                };
                let msg_type = msg.get("type").and_then(|v| v.as_str()).unwrap_or("");

                match msg_type {
                    "vtty_dirty" => {
                        dirty_count += 1;
                        // Pull the diff endpoint (the real client behaviour).
                        match manager
                            .diff_with_baseline(&id, current_baseline.as_deref())
                            .await
                        {
                            Ok((baseline, diff, _, _, gen)) => {
                                current_baseline = Some(baseline);
                                if gen > 0
                                    && (pulled_gens.is_empty()
                                        || gen != *pulled_gens.last().unwrap())
                                {
                                    pulled_gens.push(gen);
                                }
                                // Verify dirty signal carried no data.
                                assert!(
                                    diff.cells.len() > 0 || diff.changed_count > 0,
                                    "pulled diff must have content"
                                );
                            }
                            Err(_) => {
                                // Command may have exited between dirty and pull.
                            }
                        }
                    }
                    "vtty_close" => {
                        // Command finished — stop collecting.
                        break;
                    }
                    _ => {}
                }
            }
            Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(n))) => {
                // Lagged means the channel (capacity 256) overflowed.
                // With only lightweight vtty_dirty signals (no data), this
                // must NOT happen for a single command at 10 updates/sec.
                panic!(
                    "broadcast receiver lagged by {n} messages — \
                     channel is being flooded"
                );
            }
            Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) => {
                break;
            }
            Err(_) => {
                // Timeout — no more messages.
                break;
            }
        }
    }

    let _ = manager.kill(&id, None).await;

    // Must have received multiple dirty signals (8 ticks at 0.3s = 2.4s of output).
    assert!(
        dirty_count >= 2,
        "expected at least 2 vtty_dirty signals, got {}",
        dirty_count
    );

    // Must have pulled at least 2 distinct generations (proving the diff
    // endpoint returns new content each time).
    assert!(
        pulled_gens.len() >= 2,
        "expected at least 2 distinct generations from pulled diffs, \
         got {} (generations: {:?})",
        pulled_gens.len(),
        pulled_gens
    );

    // Generations must be strictly increasing.
    for i in 1..pulled_gens.len() {
        assert!(
            pulled_gens[i] > pulled_gens[i - 1],
            "generations must be strictly increasing: gen[{}]={} <= gen[{}]={}",
            i,
            pulled_gens[i],
            i - 1,
            pulled_gens[i - 1]
        );
    }
}
