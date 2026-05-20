use std::sync::Arc;
use dashmap::DashMap;
use uuid::Uuid;
use tokio::sync::broadcast;

use crate::config::schema::Config;
use crate::logging::command_log::CommandLogger;
use crate::vtty::buffer::Buffer;
use super::handle::CommandHandle;
use super::spawner::ProcessSpawner;

pub type CommandId = String;

/// Metadata stored alongside a named snapshot.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SnapshotMeta {
    pub name: String,
    pub command_id: String,
    pub command_name: String,
    pub command_args: Vec<String>,
    pub pid: u32,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub runtime_secs: f64,
}

/// A stored VTTY buffer snapshot with its metadata.
pub struct StoredSnapshot {
    pub meta: SnapshotMeta,
    pub buffer: Buffer,
}

pub struct CommandManager {
    commands: Arc<DashMap<CommandId, CommandHandle>>,
    config: Config,
    logger: Arc<CommandLogger>,
    /// Broadcast channel for VTTY change notifications.
    /// Each message is a `(command_id, html_content)` pair.
    vtty_change_tx: broadcast::Sender<(String, String)>,
    /// Named snapshots: (command_id, snapshot_name) -> StoredSnapshot
    snapshots: Arc<DashMap<(CommandId, String), StoredSnapshot>>,
    /// Last-sent buffer per command for incremental diff.
    /// Key: command_id, Value: Buffer clone from the last broadcast.
    last_buffer: Arc<DashMap<CommandId, Buffer>>,
}

impl CommandManager {
    pub fn new(config: Config) -> Self {
        let logger = Arc::new(
            CommandLogger::new(config.command_log.enabled, config.command_log.file.as_deref())
                .expect("Failed to initialize command logger")
        );
        let (vtty_change_tx, _) = broadcast::channel(256);
        Self {
            commands: Arc::new(DashMap::new()),
            config,
            logger,
            vtty_change_tx,
            snapshots: Arc::new(DashMap::new()),
            last_buffer: Arc::new(DashMap::new()),
        }
    }

    pub async fn spawn(&self, cmd: String, args: Vec<String>, certificate: Option<String>, env_vars: std::collections::HashMap<String, String>) -> anyhow::Result<CommandId> {
        let id = Uuid::new_v4().to_string();
        self.logger.log("spawn", &format!("id={} cmd={} args={:?} cert={:?} env={:?}", id, cmd, args, certificate, env_vars.keys().collect::<Vec<_>>()));

        let spawner = ProcessSpawner::new(&self.config.vtty);
        let mut handle = spawner.spawn(
            cmd,
            args,
            self.config.handles.clone(),
            &id,
            self.config.default_exit.exit.clone(),
            env_vars,
            self,
        ).await?;

        // Bind certificate to this command for per-command access control
        handle.certificate = certificate;

        self.commands.insert(id.clone(), handle);

        // Spawn a background watcher that detects VTTY changes and broadcasts them
        // using the incremental diff protocol.
        self.spawn_diff_watcher(id.clone());

        Ok(id)
    }

    /// Spawn a background watcher that computes incremental diffs and broadcasts them.
    fn spawn_diff_watcher(&self, watch_id: String) {
        let watch_commands = self.commands.clone();
        let watch_tx = self.vtty_change_tx.clone();
        let last_buf_map = self.last_buffer.clone();

        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;

                let entry = match watch_commands.get(&watch_id) {
                    Some(e) => e,
                    None => {
                        // Command removed — clean up last_buffer and stop watching
                        last_buf_map.remove(&watch_id);
                        break;
                    }
                };

                // Clone the current buffer
                let current_buffer = entry.vtty_snapshot().await;
                let cursor_row = entry.cursor_position().await.0;
                let cursor_col = entry.cursor_position().await.1;
                let (rows, cols) = entry.dimensions().await;
                let alt_screen = entry.is_alternate_screen().await;
                drop(entry); // Release the DashMap lock

                // Check if anything changed by comparing to the last sent buffer
                let prev = last_buf_map.get(&watch_id);
                let has_changed = match prev {
                    Some(p) => {
                        let p = p.value();
                        p.width != current_buffer.width
                            || p.height != current_buffer.height
                            || p.rows != current_buffer.rows
                    }
                    None => true,
                };

                if !has_changed {
                    continue;
                }

                // Compute the cell-level diff
                let diff = match prev {
                    Some(p) => current_buffer.diff(&p),
                    None => Buffer::diff(&current_buffer, &Buffer::new(0, 0, 0)),
                };

                // Only send if there are actual changes
                if diff.changed_count > 0 {
                    // Store as last sent buffer
                    last_buf_map.insert(watch_id.clone(), current_buffer);

                    // Build a JSON message containing the diff
                    let msg = serde_json::json!({
                        "type": "vtty_diff",
                        "data": {
                            "id": &watch_id,
                            "diff": diff,
                            "cursor": { "row": cursor_row, "col": cursor_col },
                            "dimensions": { "rows": rows, "cols": cols },
                            "alternate_screen": alt_screen,
                        }
                    }).to_string();

                    let _ = watch_tx.send((watch_id.clone(), msg));
                }
            }
        });
    }

    pub fn get(&self, id: &CommandId) -> Option<dashmap::mapref::one::Ref<'_, CommandId, CommandHandle>> {
        self.commands.get(id)
    }

    /// Get the certificate name bound to a command (if any).
    pub fn get_certificate(&self, id: &CommandId) -> Option<String> {
        self.commands.get(id).map(|h| h.certificate.clone()).flatten()
    }

    /// List all commands. Returns (id, name, args, pid, certificate).
    pub fn list(&self) -> Vec<(CommandId, String, Vec<String>, u32, Option<String>)> {
        self.commands
            .iter()
            .map(|entry| {
                let handle = entry.value();
                (entry.key().clone(), handle.name.clone(), handle.args.clone(), handle.pid, handle.certificate.clone())
            })
            .collect()
    }

    /// Find a command by PID.
    pub fn find_by_pid(&self, pid: u32) -> Option<CommandId> {
        self.commands
            .iter()
            .find(|entry| entry.value().pid == pid)
            .map(|entry| entry.key().clone())
    }

    /// Freeze (suspend) a command by sending SIGSTOP.
    /// The process is paused but not terminated — it can be resumed with thaw().
    pub fn freeze(&self, id: &CommandId) -> anyhow::Result<()> {
        self.logger.log("freeze", &format!("id={}", id));
        if let Some(handle) = self.commands.get(id) {
            let pid = handle.pid;
            drop(handle);
            #[cfg(unix)]
            {
                let ret = unsafe { libc::kill(pid as i32, libc::SIGSTOP) };
                if ret != 0 {
                    anyhow::bail!("Failed to freeze command {}: SIGSTOP returned {}", id, ret);
                }
            }
            #[cfg(not(unix))]
            {
                anyhow::bail!("freeze is only supported on Unix-like systems");
            }
            Ok(())
        } else {
            anyhow::bail!("Command {} not found", id)
        }
    }

    /// Thaw (resume) a frozen command by sending SIGCONT.
    pub fn thaw(&self, id: &CommandId) -> anyhow::Result<()> {
        self.logger.log("thaw", &format!("id={}", id));
        if let Some(handle) = self.commands.get(id) {
            let pid = handle.pid;
            drop(handle);
            #[cfg(unix)]
            {
                let ret = unsafe { libc::kill(pid as i32, libc::SIGCONT) };
                if ret != 0 {
                    anyhow::bail!("Failed to thaw command {}: SIGCONT returned {}", id, ret);
                }
            }
            #[cfg(not(unix))]
            {
                anyhow::bail!("thaw is only supported on Unix-like systems");
            }
            Ok(())
        } else {
            anyhow::bail!("Command {} not found", id)
        }
    }

    pub async fn kill(&self, id: &CommandId, _signal: Option<String>) -> anyhow::Result<()> {
        self.logger.log("kill", &format!("id={}", id));
        if let Some((_, handle)) = self.commands.remove(id) {
            // Clean up associated state
            self.last_buffer.remove(id);
            // Remove all snapshots for this command
            self.snapshots.retain(|k, _| k.0 != *id);

            // Send Ctrl+C (SIGINT) for graceful shutdown
            let _ = handle.send_bytes(vec![0x03]).await;

            // Wait up to the configured timeout for the process to exit
            let timeout_secs = handle.exit_config.timeout_secs;
            let exit_rx = handle._exit_rx;

            tokio::select! {
                result = tokio::time::timeout(
                    std::time::Duration::from_secs(timeout_secs),
                    exit_rx
                ) => {
                    match result {
                        Ok(Ok(_status)) => {
                            tracing::info!(id = %id, "Command exited gracefully within timeout");
                        }
                        Ok(Err(_)) => {
                            tracing::warn!(
                                id = %id,
                                timeout_secs = timeout_secs,
                                "Command did not exit within timeout, sending SIGKILL"
                            );
                            // Force kill with SIGKILL
                            #[cfg(unix)]
                            {
                                unsafe { libc::kill(handle.pid as i32, libc::SIGKILL); }
                            }
                            #[cfg(not(unix))]
                            {
                                let _ = std::process::Command::new("kill")
                                    .arg("-9").arg(handle.pid.to_string())
                                    .spawn();
                            }
                        }
                        Err(_) => {
                            tracing::warn!(id = %id, "Exit receiver dropped, sending SIGKILL");
                            #[cfg(unix)]
                            {
                                unsafe { libc::kill(handle.pid as i32, libc::SIGKILL); }
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Kill a command by its PID.
    pub async fn kill_by_pid(&self, pid: u32) -> anyhow::Result<()> {
        if let Some(id) = self.find_by_pid(pid) {
            self.kill(&id, None).await
        } else {
            anyhow::bail!("No command found with PID {}", pid)
        }
    }

    /// Store a named snapshot of a command's current VTTY buffer.
    pub fn store_snapshot(&self, id: &CommandId, name: &str) -> anyhow::Result<SnapshotMeta> {
        let entry = self.commands.get(id).ok_or_else(|| anyhow::anyhow!("Command {} not found", id))?;
        let buffer = entry.vtty_snapshot_blocking();
        let meta = SnapshotMeta {
            name: name.to_string(),
            command_id: id.clone(),
            command_name: entry.name.clone(),
            command_args: entry.args.clone(),
            pid: entry.pid,
            timestamp: chrono::Utc::now(),
            runtime_secs: entry.runtime_secs(),
        };
        self.snapshots.insert((id.clone(), name.to_string()), StoredSnapshot {
            meta: meta.clone(),
            buffer,
        });
        self.logger.log("snapshot", &format!("id={} name={}", id, name));
        Ok(meta)
    }

    /// List all snapshots for a command.
    pub fn list_snapshots(&self, id: &CommandId) -> Vec<SnapshotMeta> {
        self.snapshots
            .iter()
            .filter(|k, _| k.0 == *id)
            .map(|_, v| v.meta.clone())
            .collect()
    }

    /// List all snapshots across all commands.
    pub fn list_all_snapshots(&self) -> Vec<SnapshotMeta> {
        self.snapshots
            .iter()
            .map(|_, v| v.meta.clone())
            .collect()
    }

    /// Compute a diff of the current buffer against a stored named snapshot.
    pub fn diff_snapshot(&self, id: &CommandId, name: &str) -> anyhow::Result<crate::vtty::buffer::BufferDiff> {
        let entry = self.commands.get(id).ok_or_else(|| anyhow::anyhow!("Command {} not found", id))?;
        let current = entry.vtty_snapshot_blocking();
        drop(entry);

        let key = (id.clone(), name.to_string());
        let stored = self.snapshots.get(&key)
            .ok_or_else(|| anyhow::anyhow!("Snapshot '{}' not found for command {}", name, id))?;

        let diff = current.diff(&stored.buffer);
        self.logger.log("diff", &format!("id={} name={} changed={}", id, name, diff.changed_count));
        Ok(diff)
    }

    /// Delete a stored snapshot.
    pub fn delete_snapshot(&self, id: &CommandId, name: &str) -> anyhow::Result<()> {
        let key = (id.clone(), name.to_string());
        if self.snapshots.remove(&key).is_some() {
            self.logger.log("snapshot_delete", &format!("id={} name={}", id, name));
            Ok(())
        } else {
            anyhow::bail!("Snapshot '{}' not found for command {}", name, id)
        }
    }

    /// Subscribe to VTTY change notifications.
    /// Returns a receiver that yields `(command_id, message_json)` pairs.
    pub fn subscribe_vtty(&self) -> broadcast::Receiver<(String, String)> {
        self.vtty_change_tx.subscribe()
    }

    /// Get a clone of the VTTY change broadcast sender.
    pub fn vtty_change_sender(&self) -> broadcast::Sender<(String, String)> {
        self.vtty_change_tx.clone()
    }

    pub fn logger(&self) -> Arc<CommandLogger> {
        self.logger.clone()
    }

    /// Get a reference to the configuration (used by API handlers to access env vars etc.)
    pub fn config(&self) -> &Config {
        &self.config
    }

    pub async fn send_keys(&self, id: &CommandId, keys: &str) -> anyhow::Result<()> {
        self.logger.log("send_keys", &format!("id={} keys={}", id, keys));
        if let Some(handle) = self.commands.get(id) {
            let bytes = encode_keys(keys);
            handle.send_bytes(bytes).await?;
            Ok(())
        } else {
            anyhow::bail!("Command {} not found", id)
        }
    }

    /// Spawn a command with per-command exit configuration and environment variables.
    /// This is used by the API handler to allow on_exit/on_error/env per-command.
    pub async fn spawn_with_exit(
        &self,
        cmd: String,
        args: Vec<String>,
        certificate: Option<String>,
        on_exit: Option<String>,
        on_error: Option<String>,
        exit_timeout: u64,
        env_vars: std::collections::HashMap<String, String>,
    ) -> anyhow::Result<CommandId> {
        let id = Uuid::new_v4().to_string();
        self.logger.log("spawn", &format!("id={} cmd={} args={:?} cert={:?} env={:?}", id, cmd, args, certificate, env_vars.keys().collect::<Vec<_>>()));

        // Override default exit config with per-command values
        let exit_config = crate::config::schema::ExitConfig {
            on_exit,
            on_error,
            timeout_secs: exit_timeout,
        };

        let spawner = ProcessSpawner::new(&self.config.vtty);
        let mut handle = spawner.spawn(
            cmd,
            args,
            self.config.handles.clone(),
            &id,
            exit_config,
            env_vars,
            self,
        ).await?;

        handle.certificate = certificate;

        self.commands.insert(id.clone(), handle);

        // Spawn a background watcher that detects VTTY changes and broadcasts them
        // using the incremental diff protocol.
        self.spawn_diff_watcher(id.clone());

        Ok(id)
    }
}

pub fn encode_keys(keys: &str) -> Vec<u8> {
    let mut result = Vec::new();
    let mut chars = keys.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '<' {
            let mut seq = String::new();
            while let Some(&c) = chars.peek() {
                if c == '>' {
                    chars.next();
                    break;
                }
                seq.push(c);
                chars.next();
            }
            result.extend_from_slice(&encode_special_key(&seq));
        } else {
            result.push(ch as u8);
        }
    }

    result
}

fn encode_special_key(seq: &str) -> Vec<u8> {
    match seq {
        "Esc" => vec![0x1b],
        "Enter" | "Return" => vec![0x0d],
        "Tab" => vec![0x09],
        "Backspace" => vec![0x7f],
        "Delete" => vec![0x1b, b'[', b'3', b'~'],
        "Insert" => vec![0x1b, b'[', b'2', b'~'],
        "Home" => vec![0x1b, b'[', b'H'],
        "End" => vec![0x1b, b'[', b'F'],
        "PageUp" => vec![0x1b, b'[', b'5', b'~'],
        "PageDown" => vec![0x1b, b'[', b'6', b'~'],
        "Up" => vec![0x1b, b'[', b'A'],
        "Down" => vec![0x1b, b'[', b'B'],
        "Left" => vec![0x1b, b'[', b'D'],
        "Right" => vec![0x1b, b'[', b'C'],
        "F1" => vec![0x1b, b'[', b'1', b'1', b'~'],
        "F2" => vec![0x1b, b'[', b'1', b'2', b'~'],
        "F3" => vec![0x1b, b'[', b'1', b'3', b'~'],
        "F4" => vec![0x1b, b'[', b'1', b'4', b'~'],
        "F5" => vec![0x1b, b'[', b'1', b'5', b'~'],
        "F6" => vec![0x1b, b'[', b'1', b'7', b'~'],
        "F7" => vec![0x1b, b'[', b'1', b'8', b'~'],
        "F8" => vec![0x1b, b'[', b'1', b'9', b'~'],
        "F9" => vec![0x1b, b'[', b'2', b'0', b'~'],
        "F10" => vec![0x1b, b'[', b'2', b'1', b'~'],
        "F11" => vec![0x1b, b'[', b'2', b'3', b'~'],
        "F12" => vec![0x1b, b'[', b'2', b'4', b'~'],
        _ => {
            if let Some(rest) = seq.strip_prefix("C-") {
                if let Some(key) = rest.chars().next() {
                    let byte = if key.is_ascii_alphabetic() {
                        (key.to_ascii_uppercase() as u8) & 0x1f
                    } else {
                        match key {
                            '@' => 0x00,
                            '[' => 0x1b,
                            '\\' => 0x1c,
                            ']' => 0x1d,
                            '^' => 0x1e,
                            '_' => 0x1f,
                            '?' => 0x7f,
                            _ => key as u8,
                        }
                    };
                    return vec![byte];
                }
            }
            if let Some(rest) = seq.strip_prefix("A-") {
                if let Some(key) = rest.chars().next() {
                    return vec![0x1b, key as u8];
                }
            }
            seq.as_bytes().to_vec()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_plain_text() {
        assert_eq!(encode_keys("hello"), b"hello");
    }

    #[test]
    fn test_encode_ctrl_c() {
        assert_eq!(encode_keys("<C-c>"), vec![0x03]);
    }

    #[test]
    fn test_encode_ctrl_a() {
        assert_eq!(encode_keys("<C-a>"), vec![0x01]);
    }

    #[test]
    fn test_encode_enter() {
        assert_eq!(encode_keys("<Enter>"), vec![0x0d]);
    }

    #[test]
    fn test_encode_escape() {
        assert_eq!(encode_keys("<Esc>"), vec![0x1b]);
    }

    #[test]
    fn test_encode_arrow_keys() {
        assert_eq!(encode_keys("<Up>"), vec![0x1b, b'[', b'A']);
        assert_eq!(encode_keys("<Down>"), vec![0x1b, b'[', b'B']);
        assert_eq!(encode_keys("<Left>"), vec![0x1b, b'[', b'D']);
        assert_eq!(encode_keys("<Right>"), vec![0x1b, b'[', b'C']);
    }

    #[test]
    fn test_encode_mixed() {
        let result = encode_keys("hello<C-c>world");
        let mut expected = b"hello".to_vec();
        expected.push(0x03);
        expected.extend_from_slice(b"world");
        assert_eq!(result, expected);
    }

    #[test]
    fn test_encode_alt_key() {
        assert_eq!(encode_keys("<A-x>"), vec![0x1b, b'x']);
    }

    #[test]
    fn test_encode_delete() {
        assert_eq!(encode_keys("<Delete>"), vec![0x1b, b'[', b'3', b'~']);
    }
}
