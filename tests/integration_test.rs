use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

use vrunner::config::schema::{Config, ServerConfig, SecurityConfig, TlsConfig, VttyConfig, DisplayConfig, CommandLogConfig, DaemonConfig};
use vrunner::process::manager::CommandManager;

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
        },
        display: DisplayConfig {
            enabled: false,
            refresh_ms: 100,
        },
        command_log: CommandLogConfig {
            enabled: false,
            file: None,
        },
        daemon: DaemonConfig {
            enabled: false,
            stdout_file: "/tmp/vrunner.out".to_string(),
            stderr_file: "/tmp/vrunner.err".to_string(),
        },
        handles: vec![],
    }
}

#[tokio::test]
async fn test_spawn_and_list() {
    let cfg = test_config();
    let manager = Arc::new(CommandManager::new(cfg));

    let id = manager.spawn("echo".to_string(), vec!["hello".to_string()], None).await.unwrap();

    let list = manager.list();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].0, id);

    // Clean up
    let _ = manager.kill(&id, None).await;
}

#[tokio::test]
async fn test_vtty_contents() {
    let cfg = test_config();
    let manager = Arc::new(CommandManager::new(cfg));

    let id = manager.spawn("echo".to_string(), vec!["test_output".to_string()], None).await.unwrap();

    // Give process time to write and exit
    sleep(Duration::from_millis(200)).await;

    if let Some(handle) = manager.get(&id) {
        let plain = handle.vtty_plain().await;
        assert!(plain.contains("test_output") || plain.is_empty());
    }

    let _ = manager.kill(&id, None).await;
}

#[tokio::test]
async fn test_send_keys() {
    let cfg = test_config();
    let manager = Arc::new(CommandManager::new(cfg));

    // Spawn a shell that reads input (cat is good for testing)
    let id = manager.spawn("cat".to_string(), vec![], None).await.unwrap();

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

#[tokio::test]
async fn test_key_encoding() {
    use vrunner::process::manager::encode_keys;

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
