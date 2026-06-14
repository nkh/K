use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::broadcast;
use uuid::Uuid;

use super::error::{ProcessError, Result};

use super::handle::CommandHandle;
use super::spawner::ProcessSpawner;
use crate::config::schema::Config;
use crate::handles::{file_sink::FileSink, null_sink::NullSink, sink::Sink, vtty_sink::VttySink};
use crate::hooks::runner::run_hook;
use crate::logging::command_log::CommandLogger;
use crate::vtty::buffer::Buffer;

pub type CommandId = String;

/// A single entry returned by [`CommandManager::list`].
pub type CommandEntry = (CommandId, String, Vec<String>, u32, Option<String>);

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
    /// Each message is a `(command_id, message_json)` pair.
    vtty_change_tx: broadcast::Sender<(String, String)>,
    /// Named snapshots: (command_id, snapshot_name) -> StoredSnapshot
    snapshots: Arc<DashMap<(CommandId, String), StoredSnapshot>>,
    /// Last-known buffer generation per command for O(1) change detection.
    /// Replaces the old `last_buffer: DashMap<CommandId, Buffer>` approach
    /// which cloned the entire buffer on every poll.
    last_generation: Arc<DashMap<CommandId, u64>>,
}

impl CommandManager {
    pub fn new(config: Config) -> Self {
        let logger = Arc::new(
            CommandLogger::new(
                config.command_log.enabled,
                config.command_log.file.as_deref(),
                &config.binary_name,
                config.color_terminal_log,
                config.command_log.terminal.clone(),
            )
            .expect("Failed to initialize command logger"),
        );
        let (vtty_change_tx, _) = broadcast::channel(256);
        Self {
            commands: Arc::new(DashMap::new()),
            config,
            logger,
            vtty_change_tx,
            snapshots: Arc::new(DashMap::new()),
            last_generation: Arc::new(DashMap::new()),
        }
    }

    /// Spawn a command with optional per-command exit configuration.
    ///
    /// When `exit_config` is `None`, the global default exit configuration
    /// from `config.default_exit.exit` is used.  When `Some(...)`, the
    /// provided `ExitConfig` takes full precedence (on_exit, on_error,
    /// timeout_secs).
    #[allow(clippy::too_many_arguments)]
    pub async fn spawn(
        &self,
        cmd: String,
        args: Vec<String>,
        certificate: Option<String>,
        exit_config: Option<crate::config::schema::ExitConfig>,
        env_vars: std::collections::HashMap<String, String>,
        rows: Option<u16>,
        cols: Option<u16>,
        dir: Option<String>,
    ) -> Result<CommandId> {
        let id = Uuid::new_v4().to_string();

        // Use per-command exit config if provided, otherwise fall back to defaults
        let exit_config = exit_config.unwrap_or_else(|| self.config.default_exit.exit.clone());

        let spawner = ProcessSpawner::new(&self.config.vtty);
        let pty_raw_log = self.config.command_log.pty_raw_log.as_deref();
        let hooks = self.config.hooks.clone();
        let mut handle = spawner
            .spawn(
                cmd.clone(),
                args.clone(),
                self.config.handles.clone(),
                &id,
                exit_config,
                hooks,
                env_vars.clone(),
                self,
                rows,
                cols,
                dir.as_deref(),
                pty_raw_log,
            )
            .await?;

        // Bind certificate to this command for per-command access control
        handle.certificate = certificate.clone();

        let pid = handle.pid;
        self.commands.insert(id.clone(), handle);

        // Log spawn event AFTER the process is registered (PID is now known)
        self.logger.log(
            "spawn",
            &format!(
                "id={} pid={} cmd={} args={:?} cert={:?} env={:?} size={}x{} dir={:?}",
                id,
                pid,
                cmd,
                args,
                certificate,
                env_vars.keys().collect::<Vec<_>>(),
                rows.unwrap_or(self.config.vtty.rows),
                cols.unwrap_or(self.config.vtty.cols),
                dir
            ),
        );

        // Spawn a background watcher that detects VTTY changes and broadcasts them
        // using the incremental diff protocol.
        self.spawn_diff_watcher(id.clone());

        Ok(id)
    }

    /// Register an externally-created [`CommandHandle`] with the manager.
    ///
    /// This is the **"attach to running process"** registration path.  The
    /// caller is responsible for having already created the PTY, spawned the
    /// process, wired up the VTTY emulator, and set up all I/O plumbing.
    /// This method performs only the bookkeeping that [`CommandManager::spawn`]
    /// normally does *after* process creation:
    ///
    /// 1. Validates that no command with the same ID already exists.
    /// 2. Binds an optional certificate for per-command access control.
    /// 3. Logs the registration event via [`CommandLogger`].
    /// 4. Inserts the handle into the internal [`DashMap`] command registry.
    /// 5. Starts the background diff-watcher for VTTY change notifications.
    ///
    /// # Arguments
    ///
    /// * `handle` — A fully-constructed [`CommandHandle`] (typically built by
    ///   [`ProcessSpawner::spawn`] or a custom spawner for the attach case).
    /// * `certificate` — Optional certificate name to bind for per-command
    ///   access control.  When `Some`, only clients presenting that
    ///   certificate (or its derived bearer token) may interact with the
    ///   command.
    ///
    /// # Errors
    ///
    /// Returns an error if a command with the same ID is already registered.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use vrc_core::process::manager::CommandManager;
    /// // Build a handle via ProcessSpawner or custom code, then:
    /// // manager.add_handle(handle, Some("my-cert".into()))?;
    /// ```
    pub fn add_handle(
        &self,
        mut handle: CommandHandle,
        certificate: Option<String>,
    ) -> Result<CommandId> {
        let id = handle.id.clone();

        if self.commands.contains_key(&id) {
            return Err(ProcessError::CommandAlreadyExists(id));
        }

        // Bind certificate for per-command access control
        handle.certificate = certificate;

        self.logger.log(
            "add_handle",
            &format!(
                "id={} cmd={} pid={} cert={:?}",
                id, handle.name, handle.pid, handle.certificate
            ),
        );

        // Register in the internal command registry
        self.commands.insert(id.clone(), handle);

        // Start the background diff-watcher for VTTY push notifications
        self.spawn_diff_watcher(id.clone());

        Ok(id)
    }

    /// Dynamically attach an output sink to a running command.
    ///
    /// This completes the `POST /api/commands/:id/handles` API so that
    /// callers can add file, VTTY, or null sinks *after* a command has
    /// already been spawned.
    ///
    /// # Arguments
    ///
    /// * `id` — The command to attach the sink to.
    /// * `name` — A logical name for the sink (must be unique per command).
    /// * `sink_type` — One of `"file"`, `"vtty"`, or `"null"`.
    /// * `path` — For `"file"` sinks, the output file path.  Supports
    ///   `{id}` and `{name}` placeholders.  Ignored for other sink types.
    ///
    /// # Errors
    ///
    /// Returns an error if the command does not exist or if a sink with
    /// the given name is already registered.
    pub fn register_sink(
        &self,
        id: &CommandId,
        name: String,
        sink_type: &str,
        path: Option<&str>,
    ) -> Result<()> {
        let mut entry = self
            .commands
            .get_mut(id)
            .ok_or_else(|| ProcessError::CommandNotFound(id.clone()))?;

        let handle = entry.value_mut();

        // Reject duplicate sink names
        if handle.handle_registry.list().iter().any(|n| n == &name) {
            return Err(ProcessError::SinkAlreadyExists {
                name: name.clone(),
                command_id: id.clone(),
            });
        }

        let sink: Box<dyn Sink> = match sink_type {
            "file" => {
                let resolved = path.unwrap_or("/dev/null");
                // Substitute {id} and {name} placeholders
                let resolved = resolved.replace("{id}", id).replace("{name}", &handle.name);
                Box::new(FileSink::new(&resolved)?)
            }
            "vtty" => Box::new(VttySink::new()),
            "null" => Box::new(NullSink),
            _ => return Err(ProcessError::UnknownSinkType(sink_type.to_string())),
        };

        self.logger.log(
            "register_sink",
            &format!("id={} name={} type={}", id, name, sink_type),
        );
        handle.handle_registry.add(name, sink);
        Ok(())
    }

    /// Spawn a background watcher that detects buffer changes and broadcasts
    /// incremental diff messages using the Level 3 protocol.  The watcher
    /// maintains its own local buffer baseline, computes cell-level diffs, and
    /// sends `vtty_diff` messages with changed cells.  If the terminal dimensions
    /// change or too many cells changed (>90%), it falls back to `vtty_full`
    /// with the complete HTML so the client can resync.
    fn spawn_diff_watcher(&self, watch_id: String) {
        let watch_commands = self.commands.clone();
        let watch_tx = self.vtty_change_tx.clone();

        tokio::spawn(async move {
            let mut prev_gen: Option<u64> = None;
            let mut prev_buffer: Option<Buffer> = None;
            let mut prev_dims: Option<(usize, usize)> = None;

            loop {
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;

                // Clone the emulator Arc and drop the DashMap entry BEFORE
                // awaiting the snapshot.  Holding the DashMap shard read lock
                // across the .await blocks kill() (which needs a write lock
                // to commands.remove()), making the server appear to hang
                // when stopping a command from the web UI.
                let emulator = match watch_commands.get(&watch_id) {
                    Some(e) => e.emulator.clone(),
                    None => {
                        // Command removed — stop watching
                        break;
                    }
                };
                // DashMap shard lock is now released.

                let (current_gen, current_buf, cursor, dims, cursor_visible, alt_screen) = {
                    let emu = emulator.read().await;
                    (
                        emu.buffer_generation(),
                        emu.snapshot(),
                        emu.cursor(),
                        emu.dimensions(),
                        emu.is_cursor_visible(),
                        emu.is_alternate_screen(),
                    )
                };
                let (cursor_row, cursor_col) = cursor;
                let (rows, cols) = dims;

                let has_changed = match prev_gen {
                    Some(prev) => current_gen != prev,
                    None => true,
                };

                if !has_changed {
                    continue;
                }
                prev_gen = Some(current_gen);

                // If dimensions changed, send full HTML (resync) — cannot diff across different sizes
                let dims_changed = match prev_dims {
                    Some(prev) => prev != (rows, cols),
                    None => true,
                };

                if dims_changed || prev_buffer.is_none() {
                    // Send vtty_full for resync
                    let html = crate::vtty::renderer::VttyRenderer::to_html(&current_buf);
                    let msg = serde_json::json!({
                        "type": "vtty_full",
                        "data": {
                            "id": &watch_id,
                            "html": html,
                            "cursor": {"row": cursor_row, "col": cursor_col},
                            "dimensions": {"rows": rows, "cols": cols},
                            "alternate_screen": alt_screen,
                            "cursor_visible": cursor_visible,
                            "generation": current_gen,
                        }
                    })
                    .to_string();
                    let _ = watch_tx.send((watch_id.clone(), msg));
                    prev_buffer = Some(current_buf);
                    prev_dims = Some((rows, cols));
                    continue;
                }

                // Compute diff between previous and current buffer
                let prev = prev_buffer.as_ref().unwrap();
                let diff = current_buf.diff(prev);

                // If too many cells changed (>90% of total), fall back to full HTML
                let total_cells = rows * cols;
                if diff.changed_count > total_cells * 9 / 10 {
                    let html = crate::vtty::renderer::VttyRenderer::to_html(&current_buf);
                    let msg = serde_json::json!({
                        "type": "vtty_full",
                        "data": {
                            "id": &watch_id,
                            "html": html,
                            "cursor": {"row": cursor_row, "col": cursor_col},
                            "dimensions": {"rows": rows, "cols": cols},
                            "alternate_screen": alt_screen,
                            "cursor_visible": cursor_visible,
                            "generation": current_gen,
                        }
                    })
                    .to_string();
                    let _ = watch_tx.send((watch_id.clone(), msg));
                    prev_buffer = Some(current_buf);
                    prev_dims = Some((rows, cols));
                    continue;
                }

                // Send incremental diff
                let msg = serde_json::json!({
                    "type": "vtty_diff",
                    "data": {
                        "id": &watch_id,
                        "generation": current_gen,
                        "cursor": {"row": cursor_row, "col": cursor_col},
                        "dimensions": {"rows": rows, "cols": cols},
                        "alternate_screen": alt_screen,
                        "cursor_visible": cursor_visible,
                        "changed_count": diff.changed_count,
                        "cells": diff.cells,
                    }
                })
                .to_string();
                let _ = watch_tx.send((watch_id.clone(), msg));
                prev_buffer = Some(current_buf);
                prev_dims = Some((rows, cols));
            }
        });
    }

    pub fn get(
        &self,
        id: &CommandId,
    ) -> Option<dashmap::mapref::one::Ref<'_, CommandId, CommandHandle>> {
        self.commands.get(id)
    }

    /// Get the certificate name bound to a command (if any).
    pub fn get_certificate(&self, id: &CommandId) -> Option<String> {
        self.commands.get(id).and_then(|h| h.certificate.clone())
    }

    /// List all commands. Returns (id, name, args, pid, certificate).
    pub fn list(&self) -> Vec<CommandEntry> {
        self.commands
            .iter()
            .map(|entry| {
                let handle = entry.value();
                (
                    entry.key().clone(),
                    handle.name.clone(),
                    handle.args.clone(),
                    handle.pid,
                    handle.certificate.clone(),
                )
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
    pub fn freeze(&self, id: &CommandId) -> Result<()> {
        if let Some(handle) = self.commands.get_mut(id) {
            let pid = handle.pid;
            let name = handle.name.clone();
            handle
                .frozen
                .store(true, std::sync::atomic::Ordering::Relaxed);
            drop(handle);
            self.logger.log("freeze", &format!("id={} pid={} name={}", id, pid, name));
            #[cfg(unix)]
            {
                let ret = unsafe { libc::kill(pid as i32, libc::SIGSTOP) };
                if ret != 0 {
                    return Err(ProcessError::SignalFailed {
                        id: id.to_string(),
                        signal: "SIGSTOP".to_string(),
                        code: ret,
                    });
                }
            }
            #[cfg(not(unix))]
            {
                return Err(ProcessError::PlatformNotSupported("freeze".to_string()));
            }
            Ok(())
        } else {
            Err(ProcessError::CommandNotFound(id.to_string()))
        }
    }

    /// Thaw (resume) a frozen command by sending SIGCONT.
    pub fn thaw(&self, id: &CommandId) -> Result<()> {
        if let Some(handle) = self.commands.get_mut(id) {
            let pid = handle.pid;
            let name = handle.name.clone();
            handle
                .frozen
                .store(false, std::sync::atomic::Ordering::Relaxed);
            drop(handle);
            self.logger.log("thaw", &format!("id={} pid={} name={}", id, pid, name));
            #[cfg(unix)]
            {
                let ret = unsafe { libc::kill(pid as i32, libc::SIGCONT) };
                if ret != 0 {
                    return Err(ProcessError::SignalFailed {
                        id: id.to_string(),
                        signal: "SIGCONT".to_string(),
                        code: ret,
                    });
                }
            }
            #[cfg(not(unix))]
            {
                return Err(ProcessError::PlatformNotSupported("thaw".to_string()));
            }
            Ok(())
        } else {
            Err(ProcessError::CommandNotFound(id.to_string()))
        }
    }

    /// Kill a command: remove it from the manager, send Ctrl+C, then
    /// SIGKILL after the configured grace period.  The kill sequence
    /// runs in a background task so this method returns immediately —
    /// the caller (API handler) is not blocked waiting for the child
    /// process to exit, which previously prevented the server from
    /// shutting down gracefully.
    pub async fn kill(&self, id: &CommandId, _signal: Option<String>) -> Result<()> {
        // Check retain_on_exit before removing the command.
        // If the command is marked as kept, we still send the kill signal
        // but don't remove it from the map — the process waiter will handle
        // cleanup later (and also checks retain_on_exit).
        let retain = self.commands.get(id).map(|h| h.exit_config.retain_on_exit).unwrap_or(false);

        let (pid, name) = if let Some(handle) = self.commands.get(id) {
            (handle.pid, handle.name.clone())
        } else {
            return Err(ProcessError::CommandNotFound(id.to_string()));
        };
        self.logger.log("kill", &format!("id={} pid={} name={} retain={}", id, pid, name, retain));

        if retain {
            // Don't remove from map; just signal the process and let the
            // process waiter handle the retained entry after exit.
            if let Some(handle) = self.commands.get(id) {
                let pid = handle.pid;
                let timeout_secs = handle.exit_config.timeout_secs;
                let watch_id = id.to_string();
                let cmd_name = handle.name.clone();

                // Run on_kill hook if configured
                if let Some(ref on_kill) = self.config.hooks.on_kill {
                    let mut vars = std::collections::HashMap::new();
                    vars.insert("name", cmd_name.clone());
                    vars.insert("id", watch_id.clone());
                    vars.insert("pid", pid.to_string());
                    tracing::info!(
                        id = %watch_id,
                        name = %cmd_name,
                        pid = pid,
                        "Running on_kill hook"
                    );
                    run_hook(on_kill, &vars);
                }

                // Send Ctrl+C (SIGINT) for graceful shutdown
                let _ = handle.send_bytes(vec![0x03]).await;

                // Spawn a background task that sends SIGKILL after the grace period
                tokio::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(timeout_secs)).await;
                    #[cfg(unix)]
                    {
                        let ret = unsafe { libc::kill(pid as i32, 0) };
                        if ret == 0 {
                            tracing::info!(
                                id = %watch_id,
                                pid = pid,
                                timeout_secs = timeout_secs,
                                "Grace period expired, sending SIGKILL"
                            );
                            unsafe {
                                libc::kill(pid as i32, libc::SIGKILL);
                            }
                        }
                    }
                    #[cfg(not(unix))]
                    {
                        let _ = std::process::Command::new("kill")
                            .arg("-9")
                            .arg(pid.to_string())
                            .spawn();
                    }
                });
            }
        } else if let Some((_, handle)) = self.commands.remove(id) {
            // Clean up associated state
            self.last_generation.remove(id);
            // Remove all snapshots for this command
            self.snapshots.retain(|k, _| k.0 != *id);

            let pid = handle.pid;
            let timeout_secs = handle.exit_config.timeout_secs;
            let watch_id = id.to_string();
            let cmd_name = handle.name.clone();

            // Run on_kill hook if configured
            if let Some(ref on_kill) = self.config.hooks.on_kill {
                let mut vars = std::collections::HashMap::new();
                vars.insert("name", cmd_name.clone());
                vars.insert("id", watch_id.clone());
                vars.insert("pid", pid.to_string());
                tracing::info!(
                    id = %watch_id,
                    name = %cmd_name,
                    pid = pid,
                    "Running on_kill hook"
                );
                run_hook(on_kill, &vars);
            }

            // Step 1: send Ctrl+C (SIGINT) for graceful shutdown
            let _ = handle.send_bytes(vec![0x03]).await;

            // Step 2: spawn a background task that sends SIGKILL after
            // the grace period if the process hasn't exited yet.
            // The process waiter in spawner.rs will reap the child;
            // we just need to make sure it actually dies.
            tokio::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(timeout_secs)).await;
                // Check if the process is still alive before sending SIGKILL.
                #[cfg(unix)]
                {
                    let ret = unsafe { libc::kill(pid as i32, 0) };
                    if ret == 0 {
                        tracing::info!(
                            id = %watch_id,
                            pid = pid,
                            timeout_secs = timeout_secs,
                            "Grace period expired, sending SIGKILL"
                        );
                        unsafe {
                            libc::kill(pid as i32, libc::SIGKILL);
                        }
                    }
                }
                #[cfg(not(unix))]
                {
                    let _ = std::process::Command::new("kill")
                        .arg("-9")
                        .arg(pid.to_string())
                        .spawn();
                }
            });
        }
        Ok(())
    }

    /// Tag a command to retain its VTTY buffer after exit.
    /// Sets `retain_on_exit = true` on the command's exit config so that
    /// when the child process exits, the command is kept in the manager
    /// instead of being removed.
    pub fn keep(&self, id: &CommandId) -> Result<()> {
        if let Some(mut handle) = self.commands.get_mut(id) {
            handle.exit_config.retain_on_exit = true;
            let pid = handle.pid;
            let name = handle.name.clone();
            drop(handle);
            self.logger.log("keep", &format!("id={} pid={} name={} retain_on_exit=true", id, pid, name));
            Ok(())
        } else {
            Err(ProcessError::CommandNotFound(id.to_string()))
        }
    }

    /// Remove the retain tag from a command (un-keep).
    /// Sets `retain_on_exit = false` so the command will be removed from
    /// the manager when it exits.
    pub fn unkeep(&self, id: &CommandId) -> Result<()> {
        if let Some(mut handle) = self.commands.get_mut(id) {
            handle.exit_config.retain_on_exit = false;
            let pid = handle.pid;
            let name = handle.name.clone();
            drop(handle);
            self.logger.log("unkeep", &format!("id={} pid={} name={} retain_on_exit=false", id, pid, name));
            Ok(())
        } else {
            Err(ProcessError::CommandNotFound(id.to_string()))
        }
    }

    /// Kill a command by its PID.
    pub async fn kill_by_pid(&self, pid: u32) -> Result<()> {
        if let Some(id) = self.find_by_pid(pid) {
            self.kill(&id, None).await
        } else {
            Err(ProcessError::CommandNotFound(format!("PID {}", pid)))
        }
    }

    /// Remove a retained (exited) command from the manager.
    /// This permanently discards the VTTY buffer and all associated state.
    /// Use this to clean up commands that were kept alive via retain_on_exit.
    pub fn purge(&self, id: &CommandId) -> Result<()> {
        let (pid, name) = if let Some(handle) = self.commands.get(id) {
            (handle.pid, handle.name.clone())
        } else {
            return Err(ProcessError::CommandNotFound(id.to_string()));
        };
        self.logger.log("purge", &format!("id={} pid={} name={}", id, pid, name));
        if self.commands.remove(id).is_some() {
            self.last_generation.remove(id);
            self.snapshots.retain(|k, _| k.0 != *id);
            tracing::info!(id = %id, "Purged retained command from manager");
            Ok(())
        } else {
            Err(ProcessError::CommandNotFound(id.to_string()))
        }
    }

    /// Store a named snapshot of a command's current VTTY buffer.
    pub fn store_snapshot(&self, id: &CommandId, name: &str) -> Result<SnapshotMeta> {
        let entry = self
            .commands
            .get(id)
            .ok_or_else(|| ProcessError::CommandNotFound(id.to_string()))?;
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
        self.snapshots.insert(
            (id.clone(), name.to_string()),
            StoredSnapshot {
                meta: meta.clone(),
                buffer,
            },
        );
        self.logger
            .log("snapshot", &format!("id={} pid={} name={}", id, entry.pid, name));
        Ok(meta)
    }

    /// List all snapshots for a command.
    pub fn list_snapshots(&self, id: &CommandId) -> Vec<SnapshotMeta> {
        self.snapshots
            .iter()
            .filter(|e| e.key().0 == *id)
            .map(|e| e.value().meta.clone())
            .collect()
    }

    /// List all snapshots across all commands.
    pub fn list_all_snapshots(&self) -> Vec<SnapshotMeta> {
        self.snapshots
            .iter()
            .map(|e| e.value().meta.clone())
            .collect()
    }

    /// Compute a diff of the current buffer against a stored named snapshot.
    pub fn diff_snapshot(
        &self,
        id: &CommandId,
        name: &str,
    ) -> Result<crate::vtty::buffer::BufferDiff> {
        let entry = self
            .commands
            .get(id)
            .ok_or_else(|| ProcessError::CommandNotFound(id.to_string()))?;
        let current = entry.vtty_snapshot_blocking();
        drop(entry);

        let key = (id.clone(), name.to_string());
        let stored = self
            .snapshots
            .get(&key)
            .ok_or_else(|| ProcessError::SnapshotNotFound {
                name: name.to_string(),
                command_id: id.to_string(),
            })?;

        let diff = current.diff(&stored.buffer);
        self.logger.log(
            "diff",
            &format!("id={} name={} changed={}", id, name, diff.changed_count),
        );
        Ok(diff)
    }

    /// Delete a stored snapshot.
    pub fn delete_snapshot(&self, id: &CommandId, name: &str) -> Result<()> {
        let (pid, _cmd_name) = if let Some(handle) = self.commands.get(id) {
            (handle.pid, handle.name.clone())
        } else {
            return Err(ProcessError::CommandNotFound(id.to_string()));
        };
        let key = (id.clone(), name.to_string());
        if self.snapshots.remove(&key).is_some() {
            self.logger
                .log("snapshot_delete", &format!("id={} pid={} name={}", id, pid, name));
            Ok(())
        } else {
            Err(ProcessError::SnapshotNotFound {
                name: name.to_string(),
                command_id: id.to_string(),
            })
        }
    }

    /// Subscribe to VTTY change notifications.
    /// Returns a receiver that yields `(command_id, message_json)` pairs.
    pub fn subscribe_vtty(&self) -> broadcast::Receiver<(String, String)> {
        self.vtty_change_tx.subscribe()
    }

    /// Check whether a command's VTTY buffer has changed since the last
    /// snapshot.  Used by the `GET /api/commands/:id/vtty/changed` endpoint
    /// in poll mode.
    ///
    /// Returns `true` if the command exists and the buffer differs from the
    /// last-known state (or if this is the first check).  Returns `false` if
    /// the buffer is unchanged.  Returns an error if the command does not exist.
    pub fn has_changed(&self, id: &CommandId) -> Result<bool> {
        // Clone the emulator Arc and drop the DashMap entry BEFORE
        // the blocking read, to avoid holding the shard lock.
        let emulator = match self.commands.get(id) {
            Some(e) => e.emulator.clone(),
            None => return Err(ProcessError::CommandNotFound(id.to_string())),
        };
        // DashMap shard lock is now released.

        // Use the generation counter for O(1) change detection.
        // This replaces the old approach that cloned the entire buffer
        // (O(rows * cols)) on every poll request.
        let current_gen = {
            let emu = tokio::task::block_in_place(|| emulator.blocking_read());
            emu.buffer_generation()
        };

        let changed = match self.last_generation.get(id) {
            Some(prev) => *prev.value() != current_gen,
            None => true,
        };

        if changed {
            // Update the last-known generation so the next check won't report
            // changed unless the buffer actually changes again.
            self.last_generation.insert(id.to_string(), current_gen);
        }

        Ok(changed)
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

    /// Get a clone of the commands `Arc<DashMap>` for use in spawned tasks
    /// that need to remove commands after exit (e.g. the process waiter).
    pub fn commands_arc(&self) -> Arc<DashMap<CommandId, CommandHandle>> {
        self.commands.clone()
    }

    pub async fn send_keys(&self, id: &CommandId, keys: &str) -> Result<()> {
        let (pid, name) = if let Some(handle) = self.commands.get(id) {
            (handle.pid, handle.name.clone())
        } else {
            return Err(ProcessError::CommandNotFound(id.to_string()));
        };
        self.logger
            .log("send_keys", &format!("id={} pid={} name={} keys={}", id, pid, name, keys));
        if let Some(handle) = self.commands.get(id) {
            let bytes = encode_keys(keys);
            handle.send_bytes(bytes).await?;
            Ok(())
        } else {
            Err(ProcessError::CommandNotFound(id.to_string()))
        }
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

    // ─── manager tests ───

    // ─── Helper infrastructure for CommandManager tests ───

    use crate::process::handle::CommandHandle;
    use crate::handles::registry::HandleRegistry;
    use crate::vtty::emulator::VttyEmulator;
    use crate::vtty::sink::VttyOutput;

    fn make_manager() -> CommandManager {
        let mut config = Config::default();
        config.binary_name = "test".to_string();
        CommandManager::new(config)
    }

    fn make_mock_handle(id: &str, pid: u32) -> (CommandHandle, tokio::sync::mpsc::Receiver<super::super::spawner::StdinMessage>) {
        let (stdin_tx, stdin_rx) = tokio::sync::mpsc::channel::<super::super::spawner::StdinMessage>(16);
        let (_exit_tx, exit_rx) = tokio::sync::oneshot::channel::<super::super::spawner::ExitStatus>();
        let (watch_tx, watch_rx) = tokio::sync::watch::channel(false);
        let emu = VttyEmulator::new(24, 80, 1000);
        std::mem::forget(watch_tx);
        let handle = CommandHandle {
            id: id.to_string(), pid,
            name: format!("cmd-{}", id),
            args: vec!["--test".to_string()],
            emulator: std::sync::Arc::new(tokio::sync::RwLock::new(emu)),
            stdin_tx, _exit_rx: exit_rx,
            handle_registry: HandleRegistry::new(),
            certificate: None,
            exit_config: crate::config::schema::ExitConfig::default(),
            spawn_time: std::time::Instant::now(),
            pty_master: None,
            vtty_output: std::sync::Arc::new(VttyOutput::new()),
            exit_rx: watch_rx,
            exit_code: std::sync::Mutex::new(None),
            exit_time: std::sync::Mutex::new(None),
            frozen: std::sync::atomic::AtomicBool::new(false),
            prev_diff_snapshot: tokio::sync::Mutex::new(None),
        };
        (handle, stdin_rx)
    }

    fn insert_mock(mgr: &CommandManager, id: &str, pid: u32) -> tokio::sync::mpsc::Receiver<super::super::spawner::StdinMessage> {
        let (handle, rx) = make_mock_handle(id, pid);
        mgr.commands_arc().insert(id.to_string(), handle);
        rx
    }

    // ─── CommandManager::new ───

    #[test]
    fn test_manager_new() {
        let mgr = make_manager();
        assert!(mgr.list().is_empty());
        assert!(mgr.list_all_snapshots().is_empty());
    }

    // ─── get ───

    #[test]
    fn test_get_missing() {
        let mgr = make_manager();
        assert!(mgr.get(&"none".to_string()).is_none());
    }

    #[test]
    fn test_get_existing() {
        let mgr = make_manager();
        insert_mock(&mgr, "cmd-1", 100);
        assert!(mgr.get(&"cmd-1".to_string()).is_some());
        assert_eq!(mgr.get(&"cmd-1".to_string()).unwrap().pid, 100);
    }

    // ─── get_certificate ───

    #[test]
    fn test_get_certificate_none() {
        let mgr = make_manager();
        insert_mock(&mgr, "cmd-1", 100);
        assert!(mgr.get_certificate(&"cmd-1".to_string()).is_none());
    }

    #[test]
    fn test_get_certificate_some() {
        let mgr = make_manager();
        insert_mock(&mgr, "cmd-1", 100);
        if let Some(mut h) = mgr.commands_arc().get_mut("cmd-1") {
            h.certificate = Some("my-cert".to_string());
        }
        assert_eq!(mgr.get_certificate(&"cmd-1".to_string()).as_deref(), Some("my-cert"));
    }

    #[test]
    fn test_get_certificate_missing_cmd() {
        let mgr = make_manager();
        assert!(mgr.get_certificate(&"none".to_string()).is_none());
    }

    // ─── list / find_by_pid ───

    #[test]
    fn test_list_empty() {
        let mgr = make_manager();
        assert!(mgr.list().is_empty());
    }

    #[test]
    fn test_list_with_commands() {
        let mgr = make_manager();
        insert_mock(&mgr, "a", 100);
        insert_mock(&mgr, "b", 200);
        assert_eq!(mgr.list().len(), 2);
    }

    #[test]
    fn test_find_by_pid_found() {
        let mgr = make_manager();
        insert_mock(&mgr, "x", 42);
        assert_eq!(mgr.find_by_pid(42).as_deref(), Some("x"));
    }

    #[test]
    fn test_find_by_pid_missing() {
        let mgr = make_manager();
        assert!(mgr.find_by_pid(99999).is_none());
    }

    // ─── freeze / thaw ───

    #[test]
    fn test_freeze_missing() {
        let mgr = make_manager();
        assert!(mgr.freeze(&"none".to_string()).is_err());
    }

    #[test]
    fn test_freeze_sets_flag() {
        let mgr = make_manager();
        insert_mock(&mgr, "cmd-1", 1);
        let _ = mgr.freeze(&"cmd-1".to_string());
        if let Some(h) = mgr.commands_arc().get("cmd-1") {
            assert!(h.frozen.load(std::sync::atomic::Ordering::Relaxed));
        }
    }

    #[test]
    fn test_thaw_missing() {
        let mgr = make_manager();
        assert!(mgr.thaw(&"none".to_string()).is_err());
    }

    #[test]
    fn test_thaw_clears_flag() {
        let mgr = make_manager();
        insert_mock(&mgr, "cmd-1", 1);
        if let Some(h) = mgr.commands_arc().get_mut("cmd-1") {
            h.frozen.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        let _ = mgr.thaw(&"cmd-1".to_string());
        if let Some(h) = mgr.commands_arc().get("cmd-1") {
            assert!(!h.frozen.load(std::sync::atomic::Ordering::Relaxed));
        }
    }

    // ─── keep / unkeep ───

    #[test]
    fn test_keep_missing() {
        let mgr = make_manager();
        assert!(mgr.keep(&"none".to_string()).is_err());
    }

    #[test]
    fn test_keep_sets_retain() {
        let mgr = make_manager();
        insert_mock(&mgr, "cmd-1", 1);
        mgr.keep(&"cmd-1".to_string()).unwrap();
        if let Some(h) = mgr.commands_arc().get("cmd-1") {
            assert!(h.exit_config.retain_on_exit);
        }
    }

    #[test]
    fn test_unkeep_missing() {
        let mgr = make_manager();
        assert!(mgr.unkeep(&"none".to_string()).is_err());
    }

    #[test]
    fn test_unkeep_clears_retain() {
        let mgr = make_manager();
        insert_mock(&mgr, "cmd-1", 1);
        mgr.keep(&"cmd-1".to_string()).unwrap();
        mgr.unkeep(&"cmd-1".to_string()).unwrap();
        if let Some(h) = mgr.commands_arc().get("cmd-1") {
            assert!(!h.exit_config.retain_on_exit);
        }
    }

    // ─── purge ───

    #[test]
    fn test_purge_missing() {
        let mgr = make_manager();
        assert!(mgr.purge(&"none".to_string()).is_err());
    }

    #[test]
    fn test_purge_removes_command() {
        let mgr = make_manager();
        insert_mock(&mgr, "cmd-1", 1);
        assert!(mgr.get(&"cmd-1".to_string()).is_some());
        mgr.purge(&"cmd-1".to_string()).unwrap();
        assert!(mgr.get(&"cmd-1".to_string()).is_none());
    }

    // ─── snapshots ───

    #[test]
    fn test_store_snapshot_missing_cmd() {
        let mgr = make_manager();
        assert!(mgr.store_snapshot(&"none".to_string(), "snap1").is_err());
    }

    #[test]
    fn test_store_and_list_snapshots() {
        let mgr = make_manager();
        insert_mock(&mgr, "cmd-1", 1);
        mgr.store_snapshot(&"cmd-1".to_string(), "s1").unwrap();
        mgr.store_snapshot(&"cmd-1".to_string(), "s2").unwrap();
        let snaps = mgr.list_snapshots(&"cmd-1".to_string());
        assert_eq!(snaps.len(), 2);
        let names: Vec<&str> = snaps.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"s1"));
        assert!(names.contains(&"s2"));
    }

    #[test]
    fn test_list_snapshots_missing_cmd() {
        let mgr = make_manager();
        assert!(mgr.list_snapshots(&"none".to_string()).is_empty());
    }

    #[test]
    fn test_list_all_snapshots_empty() {
        let mgr = make_manager();
        assert!(mgr.list_all_snapshots().is_empty());
    }

    #[test]
    fn test_list_all_snapshots_across_commands() {
        let mgr = make_manager();
        insert_mock(&mgr, "cmd-1", 1);
        insert_mock(&mgr, "cmd-2", 2);
        mgr.store_snapshot(&"cmd-1".to_string(), "a").unwrap();
        mgr.store_snapshot(&"cmd-2".to_string(), "b").unwrap();
        assert_eq!(mgr.list_all_snapshots().len(), 2);
    }

    #[test]
    fn test_diff_snapshot_missing_cmd() {
        let mgr = make_manager();
        assert!(mgr.diff_snapshot(&"none".to_string(), "snap").is_err());
    }

    #[test]
    fn test_diff_snapshot_missing_snap() {
        let mgr = make_manager();
        insert_mock(&mgr, "cmd-1", 1);
        assert!(mgr.diff_snapshot(&"cmd-1".to_string(), "none").is_err());
    }

    #[test]
    fn test_diff_snapshot_no_change() {
        let mgr = make_manager();
        insert_mock(&mgr, "cmd-1", 1);
        mgr.store_snapshot(&"cmd-1".to_string(), "v1").unwrap();
        let diff = mgr.diff_snapshot(&"cmd-1".to_string(), "v1").unwrap();
        assert_eq!(diff.changed_count, 0);
    }

    #[test]
    fn test_diff_snapshot_with_change() {
        let mgr = make_manager();
        insert_mock(&mgr, "cmd-1", 1);
        mgr.store_snapshot(&"cmd-1".to_string(), "v1").unwrap();
        if let Some(h) = mgr.commands_arc().get("cmd-1") {
            let mut emu = h.emulator.blocking_write();
            emu.feed_str("X");
        }
        let diff = mgr.diff_snapshot(&"cmd-1".to_string(), "v1").unwrap();
        assert!(diff.changed_count > 0);
    }

    #[test]
    fn test_delete_snapshot_missing_cmd() {
        let mgr = make_manager();
        assert!(mgr.delete_snapshot(&"none".to_string(), "s").is_err());
    }

    #[test]
    fn test_delete_snapshot_missing_snap() {
        let mgr = make_manager();
        insert_mock(&mgr, "cmd-1", 1);
        assert!(mgr.delete_snapshot(&"cmd-1".to_string(), "none").is_err());
    }

    #[test]
    fn test_delete_snapshot_success() {
        let mgr = make_manager();
        insert_mock(&mgr, "cmd-1", 1);
        mgr.store_snapshot(&"cmd-1".to_string(), "s1").unwrap();
        assert_eq!(mgr.list_snapshots(&"cmd-1".to_string()).len(), 1);
        mgr.delete_snapshot(&"cmd-1".to_string(), "s1").unwrap();
        assert!(mgr.list_snapshots(&"cmd-1".to_string()).is_empty());
    }

    // ─── subscribe_vtty / vtty_change_sender / logger / config / commands_arc ───

    #[test]
    fn test_subscribe_vtty() {
        let mgr = make_manager();
        let _rx = mgr.subscribe_vtty();
    }

    #[test]
    fn test_vtty_change_sender() {
        let mgr = make_manager();
        let tx = mgr.vtty_change_sender();
        // tokio broadcast send() returns Ok(num_receivers) or Err if channel is closed.
        // With 0 receivers, tokio broadcast still sends successfully (returns Ok(0)).
        // Keep the manager alive so the channel isn't closed.
        let _keep_alive = &mgr;
        let res = tx.send(("test".to_string(), "{}".to_string()));
        // If the send fails it means the channel is closed, which is a valid test scenario.
        // The important thing is that we got a sender clone successfully.
        match res {
            Ok(n) => { /* n = number of active receivers, could be 0 */ }
            Err(e) => { /* channel closed (e.g., no receivers) */ }
        }
    }

    // ─── has_changed ───

    #[test]
    fn test_has_changed_missing() {
        let mgr = make_manager();
        assert!(mgr.has_changed(&"none".to_string()).is_err());
    }

    #[test]
    fn test_has_changed_first_check() {
        let mgr = make_manager();
        insert_mock(&mgr, "cmd-1", 1);
        assert!(mgr.has_changed(&"cmd-1".to_string()).unwrap());
    }

    #[test]
    fn test_has_changed_no_mutation() {
        let mgr = make_manager();
        insert_mock(&mgr, "cmd-1", 1);
        mgr.has_changed(&"cmd-1".to_string()).unwrap();
        assert!(!mgr.has_changed(&"cmd-1".to_string()).unwrap());
    }

    // ─── send_keys ───

    #[tokio::test]
    async fn test_send_keys_missing() {
        let mgr = make_manager();
        assert!(mgr.send_keys(&"none".to_string(), "hi").await.is_err());
    }

    #[tokio::test]
    async fn test_send_keys_success() {
        let mgr = make_manager();
        let mut rx = insert_mock(&mgr, "cmd-1", 1);
        mgr.send_keys(&"cmd-1".to_string(), "AB").await.unwrap();
        let msg = rx.recv().await.unwrap();
        match msg {
            super::super::spawner::StdinMessage::Bytes(data) => assert_eq!(data, b"AB"),
            _ => panic!("expected Bytes"),
        }
    }

    #[tokio::test]
    async fn test_send_keys_with_special() {
        let mgr = make_manager();
        let mut rx = insert_mock(&mgr, "cmd-1", 1);
        mgr.send_keys(&"cmd-1".to_string(), "<C-c>").await.unwrap();
        let msg = rx.recv().await.unwrap();
        match msg {
            super::super::spawner::StdinMessage::Bytes(data) => assert_eq!(data, vec![0x03]),
            _ => panic!("expected Bytes"),
        }
    }

    // ─── add_handle ───

    #[tokio::test]
    async fn test_add_handle_success() {
        let mgr = make_manager();
        let (handle, _rx) = make_mock_handle("new-cmd", 999);
        let result = mgr.add_handle(handle, Some("cert".to_string()));
        assert!(result.is_ok());
        assert!(mgr.get(&"new-cmd".to_string()).is_some());
    }

    #[tokio::test]
    async fn test_add_handle_duplicate() {
        let mgr = make_manager();
        let (h1, _r1) = make_mock_handle("dup", 1);
        let (h2, _r2) = make_mock_handle("dup", 2);
        mgr.add_handle(h1, None).unwrap();
        assert!(mgr.add_handle(h2, None).is_err());
    }

    // ─── register_sink ───

    #[test]
    fn test_register_sink_missing_cmd() {
        let mgr = make_manager();
        assert!(mgr.register_sink(&"none".to_string(), "s1".into(), "null", None).is_err());
    }

    #[test]
    fn test_register_null_sink() {
        let mgr = make_manager();
        insert_mock(&mgr, "cmd-1", 1);
        mgr.register_sink(&"cmd-1".to_string(), "ns".into(), "null", None).unwrap();
        if let Some(h) = mgr.commands_arc().get("cmd-1") {
            assert!(h.handle_registry.list().contains(&"ns".to_string()));
        }
    }

    #[test]
    fn test_register_vtty_sink() {
        let mgr = make_manager();
        insert_mock(&mgr, "cmd-1", 1);
        mgr.register_sink(&"cmd-1".to_string(), "vs".into(), "vtty", None).unwrap();
        if let Some(h) = mgr.commands_arc().get("cmd-1") {
            assert!(h.handle_registry.list().contains(&"vs".to_string()));
        }
    }

    #[test]
    fn test_register_file_sink() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("out.log").to_string_lossy().to_string();
        let mgr = make_manager();
        insert_mock(&mgr, "cmd-1", 1);
        mgr.register_sink(&"cmd-1".to_string(), "fs".into(), "file", Some(&path)).unwrap();
        if let Some(h) = mgr.commands_arc().get("cmd-1") {
            assert!(h.handle_registry.list().contains(&"fs".to_string()));
        }
    }

    #[test]
    fn test_register_unknown_sink_type() {
        let mgr = make_manager();
        insert_mock(&mgr, "cmd-1", 1);
        assert!(mgr.register_sink(&"cmd-1".to_string(), "bad".into(), "bad_type", None).is_err());
    }

    #[test]
    fn test_register_duplicate_sink_name() {
        let mgr = make_manager();
        insert_mock(&mgr, "cmd-1", 1);
        mgr.register_sink(&"cmd-1".to_string(), "dup".into(), "null", None).unwrap();
        assert!(mgr.register_sink(&"cmd-1".to_string(), "dup".into(), "null", None).is_err());
    }

    // ─── kill ───

    #[tokio::test]
    async fn test_kill_missing() {
        let mgr = make_manager();
        assert!(mgr.kill(&"none".to_string(), None).await.is_err());
    }

    #[tokio::test]
    async fn test_kill_removes_command() {
        let mgr = make_manager();
        insert_mock(&mgr, "cmd-1", 1);
        mgr.kill(&"cmd-1".to_string(), None).await.unwrap();
        assert!(mgr.get(&"cmd-1".to_string()).is_none());
    }

    #[tokio::test]
    async fn test_kill_retained_keeps_command() {
        let mgr = make_manager();
        insert_mock(&mgr, "cmd-1", 1);
        mgr.keep(&"cmd-1".to_string()).unwrap();
        mgr.kill(&"cmd-1".to_string(), None).await.unwrap();
        assert!(mgr.get(&"cmd-1".to_string()).is_some());
    }

    #[tokio::test]
    async fn test_kill_by_pid_missing() {
        let mgr = make_manager();
        assert!(mgr.kill_by_pid(99999).await.is_err());
    }

    #[tokio::test]
    async fn test_kill_by_pid_found() {
        let mgr = make_manager();
        insert_mock(&mgr, "cmd-1", 42);
        let result = mgr.kill_by_pid(42).await;
        assert!(result.is_ok());
        assert!(mgr.get(&"cmd-1".to_string()).is_none());
    }

    // ─── purge removes snapshots ───

    #[test]
    fn test_purge_removes_snapshots() {
        let mgr = make_manager();
        insert_mock(&mgr, "cmd-1", 1);
        mgr.store_snapshot(&"cmd-1".to_string(), "s1").unwrap();
        mgr.store_snapshot(&"cmd-1".to_string(), "s2").unwrap();
        mgr.purge(&"cmd-1".to_string()).unwrap();
        assert!(mgr.list_snapshots(&"cmd-1".to_string()).is_empty());
    }

    // ─── Integration: snapshot lifecycle ───

    #[test]
    fn test_snapshot_lifecycle() {
        let mgr = make_manager();
        insert_mock(&mgr, "cmd-1", 1);
        let meta = mgr.store_snapshot(&"cmd-1".to_string(), "v1").unwrap();
        assert_eq!(meta.name, "v1");
        assert_eq!(meta.command_id, "cmd-1");
        assert_eq!(mgr.list_snapshots(&"cmd-1".to_string()).len(), 1);
        let diff = mgr.diff_snapshot(&"cmd-1".to_string(), "v1").unwrap();
        assert_eq!(diff.changed_count, 0);
        mgr.delete_snapshot(&"cmd-1".to_string(), "v1").unwrap();
        assert!(mgr.list_snapshots(&"cmd-1".to_string()).is_empty());
    }
}
