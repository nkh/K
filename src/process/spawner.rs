use std::fmt::Write as FmtWrite;
use std::io::{Read, Write};

use super::error::Result;
use super::pty::openpty;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};

use std::collections::HashMap;

use super::handle::CommandHandle;
use crate::config::schema::{ExitConfig, HandleConfig, HooksConfig, VttyConfig};
use crate::handles::{
    file_sink::FileSink, null_sink::NullSink, registry::HandleRegistry, sink::Sink,
    vtty_sink::VttySink,
};
use crate::hooks::runner::run_hook;
use crate::process::manager::CommandManager;
use crate::vtty::buffer::Buffer;
use crate::vtty::emulator::VttyEmulator;
use crate::vtty::rate_limiter::RateLimiter;
use crate::vtty::sink::{BroadcastVttySink, VttyOutput};

/// Default rate limit for VTTY output notifications (updates per second).
const DEFAULT_RATE_LIMIT: u32 = 30;

pub struct ProcessSpawner {
    vtty_cfg: VttyConfig,
    max_updates_per_sec: u32,
}

pub enum StdinMessage {
    Bytes(Vec<u8>),
    Signal(String),
}

/// Internal message bridging the sync PTY reader to the async emulator writer.
/// Each message carries a chunk of bytes read from the PTY master fd.
struct PtyOutput(Vec<u8>);

/// Exit status reported when a child process terminates.
#[derive(Debug, Clone)]
pub struct ExitStatus {
    /// Process exit code. None if the process was killed by a signal.
    pub code: Option<i32>,
    /// Signal number that killed the process, if applicable.
    pub signal: Option<i32>,
}

impl ExitStatus {
    pub fn success(&self) -> bool {
        self.code == Some(0)
    }

    pub fn is_error(&self) -> bool {
        !self.success()
    }
}

impl ProcessSpawner {
    pub fn new(vtty_cfg: &VttyConfig) -> Self {
        Self {
            vtty_cfg: vtty_cfg.clone(),
            max_updates_per_sec: DEFAULT_RATE_LIMIT,
        }
    }

    /// Build the handle registry from per-command handle configurations.
    ///
    /// Each handle config specifies a named sink (file, vtty, or null).
    /// Placeholders `{id}` and `{name}` in file paths are substituted.
    fn build_handle_registry(
        &self,
        handle_configs: Vec<HandleConfig>,
        command_id: &str,
        cmd: &str,
    ) -> Result<HandleRegistry> {
        let mut handle_registry = HandleRegistry::new();
        for cfg in handle_configs {
            let sink: Box<dyn Sink> = match cfg.sink.as_str() {
                "file" => {
                    let path = cfg.path.as_deref().unwrap_or("/dev/null");
                    // Substitute placeholders
                    let path = path.replace("{id}", command_id).replace("{name}", cmd);
                    Box::new(FileSink::new(&path)?)
                }
                "vtty" => Box::new(VttySink::new()),
                "null" => Box::new(NullSink),
                _ => Box::new(NullSink),
            };
            handle_registry.add(cfg.name, sink);
        }
        Ok(handle_registry)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn spawn(
        &self,
        cmd: String,
        args: Vec<String>,
        handle_configs: Vec<HandleConfig>,
        command_id: &str,
        exit_config: ExitConfig,
        hooks: HooksConfig,
        env_vars: HashMap<String, String>,
        manager: &CommandManager,
        rows: Option<u16>,
        cols: Option<u16>,
        dir: Option<&str>,
        pty_raw_log: Option<&str>,
    ) -> Result<CommandHandle> {
        // Architecture: orchestrates process lifecycle in 5 phases:
        //   1. Open PTY + fork child process
        //   2. Build handle registry (file/vtty/null sinks)
        //   3. Spawn 3 async tasks: PTY reader, emulator writer, stdin writer
        //   4. Spawn process waiter (exit hooks, snapshot, cleanup)
        //   5. Return CommandHandle with all channels and state

        // --- Phase 1: Open PTY + fork child process ---
        let _cmd_display = cmd.clone(); // for error reporting
                                        // Use per-command overrides if provided, otherwise fall back to config defaults
        let rows = rows.unwrap_or(self.vtty_cfg.rows);
        let cols = cols.unwrap_or(self.vtty_cfg.cols);

        let pair = openpty(rows, cols)?;

        // Spawn the child process via the PTY slave
        let child = pair
            .slave
            .spawn_command(&cmd, &args, &self.vtty_cfg.term, &env_vars, dir)?;
        let pid = child.process_id().unwrap_or(0);

        // Run on_spawn hook if configured
        if let Some(ref on_spawn) = hooks.on_spawn {
            let mut vars = HashMap::new();
            vars.insert("name", cmd.clone());
            vars.insert("id", command_id.to_string());
            vars.insert("pid", pid.to_string());
            tracing::info!(
                id = %command_id,
                name = %cmd,
                pid = pid,
                "Running on_spawn hook"
            );
            run_hook(on_spawn, &vars);
        }

        // Create VTTY emulator
        let emulator = Arc::new(tokio::sync::RwLock::new(VttyEmulator::new(
            rows,
            cols,
            self.vtty_cfg.scrollback,
        )));

        // --- Phase 2: Build handle registry ---
        let handle_registry = self.build_handle_registry(handle_configs, command_id, &cmd)?;

        // --- Phase 3: Set up channels and spawn 3 async tasks ---

        // Channel for stdin injection (async → blocking)
        let (stdin_tx, stdin_rx) = mpsc::channel::<StdinMessage>(128);

        // Channel for PTY output (blocking → async)
        // Uses a bounded channel to provide backpressure: if the async consumer
        // (emulator writer) is slow, the blocking reader will naturally wait.
        let (pty_out_tx, pty_out_rx) = mpsc::channel::<PtyOutput>(64);

        // Get PTY master reader and writer
        let reader = pair.master.clone_reader()?;
        let writer = pair.master.take_writer()?;

        // Store the PTY master handle for later resize (e.g. WINCH handling).
        let pty_master: Arc<parking_lot::Mutex<crate::process::pty::PtyMaster>> =
            Arc::new(parking_lot::Mutex::new(pair.master));

        // Spawn PTY reader task (blocking thread)
        spawn_pty_reader(reader, pty_out_tx, pty_raw_log);

        // Create VttyOutput with a BroadcastVttySink that pushes dirty
        // notifications via the same broadcast channel used by the existing
        // diff watcher.  This provides an immediate, push-based notification
        // path that complements the polling-based watcher.
        let vtty_output: Arc<VttyOutput> = Arc::new(VttyOutput::with_sinks(vec![Arc::new(
            BroadcastVttySink::new(manager.vtty_change_sender(), command_id.to_string()),
        )]));

        // Wrap PTY writer in Arc<Mutex> for shared access between the stdin
        // writer task and the PTY output consumer (which needs to write
        // emulator responses like DA1 replies back to the child PTY).
        let writer = Arc::new(parking_lot::Mutex::new(writer));

        // Spawn async PTY output consumer — feeds data into the emulator
        // and notifies all registered VttySinks, with rate limiting.
        spawn_emulator_writer(
            emulator.clone(),
            pty_out_rx,
            vtty_output.clone(),
            writer.clone(),
            self.max_updates_per_sec,
        );

        // Spawn stdin writer task (blocking thread)
        spawn_stdin_writer(stdin_rx, writer);

        // --- Phase 4: Spawn process waiter ---
        let (exit_tx, exit_rx) = oneshot::channel::<ExitStatus>();
        let watch_id = command_id.to_string();
        let on_exit = exit_config.on_exit.clone();
        let on_error = exit_config.on_error.clone();
        let global_on_exit = hooks.on_exit.clone();
        let global_on_error = hooks.on_error.clone();
        let manager_cmds = manager.commands_arc();
        let logger = manager.logger();
        let (child_exit_tx, child_exit_rx) = tokio::sync::watch::channel(false);
        let snapshot_on_exit = exit_config.snapshot_on_exit.clone();
        let snapshot_emulator = emulator.clone();

        spawn_process_waiter(
            child,
            watch_id,
            on_exit,
            on_error,
            global_on_exit,
            global_on_error,
            manager_cmds,
            logger,
            snapshot_emulator,
            snapshot_on_exit,
            child_exit_tx,
            exit_tx,
        );

        // --- Phase 5: Return CommandHandle ---
        Ok(CommandHandle {
            id: command_id.to_string(),
            pid,
            name: cmd.clone(),
            args: args.clone(),
            emulator,
            stdin_tx,
            _exit_rx: exit_rx,
            handle_registry,
            certificate: None,
            exit_config,
            spawn_time: std::time::Instant::now(),
            pty_master: Some(pty_master),
            vtty_output,
            exit_rx: child_exit_rx,
            exit_code: std::sync::Mutex::new(None),
            exit_time: std::sync::Mutex::new(None),
            frozen: std::sync::atomic::AtomicBool::new(false),
            prev_diff_snapshot: tokio::sync::Mutex::new(None),
        })
    }
}

// ---------------------------------------------------------------------------
// Extracted helper functions
// ---------------------------------------------------------------------------

/// Spawn the blocking PTY reader task.
///
/// Reads raw bytes from the PTY master fd in a blocking thread and forwards
/// them to the async emulator writer via a bounded channel.  When
/// `pty_raw_log` is set, each read chunk is logged in escaped hex format
/// with an elapsed-time stamp.
fn spawn_pty_reader(
    reader: Box<dyn Read + Send>,
    pty_out_tx: mpsc::Sender<PtyOutput>,
    pty_raw_log: Option<&str>,
) {
    let pty_raw_log_owned = pty_raw_log.map(|s| s.to_string());
    tokio::task::spawn_blocking(move || {
        let mut reader = reader;
        let mut buf = [0u8; 4096];
        let start = std::time::Instant::now();

        // Open the raw PTY log file if configured.
        let mut log_file: Option<std::fs::File> =
            pty_raw_log_owned
                .as_deref()
                .and_then(|path| match std::fs::File::create(path) {
                    Ok(f) => {
                        tracing::info!(path = path, "PTY raw log opened");
                        Some(f)
                    }
                    Err(e) => {
                        tracing::warn!(path = path, error = %e,
                        "Failed to open PTY raw log, skipping");
                        None
                    }
                });

        loop {
            match reader.read(&mut buf) {
                Ok(0) => break, // EOF — child process closed
                Ok(n) => {
                    let data = buf[..n].to_vec();

                    // Write to raw PTY log if configured.
                    if let Some(ref mut f) = log_file {
                        let elapsed = start.elapsed().as_millis();
                        let _ = writeln!(f, "{:06} {}", elapsed, escape_bytes(&data));
                    }

                    let pty_data = PtyOutput(data);
                    // blocking_send will wait if the channel is full,
                    // providing natural backpressure against fast PTY output.
                    if pty_out_tx.blocking_send(pty_data).is_err() {
                        // Receiver dropped — emulator task gone, shut down reader
                        break;
                    }
                }
                Err(_) => break, // PTY read error
            }
        }
    });
}

/// Spawn the async PTY output consumer task.
///
/// Feeds PTY output into the VTTY emulator and notifies all registered
/// VttySinks.  Supports two modes:
/// - **Unlimited**: every emulator snapshot is immediately pushed to sinks.
/// - **Rate-limited**: snapshots are batched and flushed at a configurable
///   interval to avoid overwhelming downstream consumers.
fn spawn_emulator_writer(
    emulator: Arc<tokio::sync::RwLock<VttyEmulator>>,
    mut pty_out_rx: mpsc::Receiver<PtyOutput>,
    vtty_output: Arc<VttyOutput>,
    writer: Arc<parking_lot::Mutex<Box<dyn Write + Send>>>,
    max_updates_per_sec: u32,
) {
    let mut rate_limiter = RateLimiter::from_config(max_updates_per_sec);
    let flush_interval = rate_limiter.interval();
    tokio::spawn(async move {
        // State for rate-limited buffering.
        let mut pending_snapshot: Option<Buffer> = None;
        // If rate limiting is disabled, use a simple loop (no timer overhead).
        if rate_limiter.is_disabled() {
            while let Some(PtyOutput(data)) = pty_out_rx.recv().await {
                let mut emu = emulator.write().await;
                emu.feed(&data);
                let responses = emu.drain_responses();
                let snapshot = emu.snapshot();
                drop(emu);
                // Send any pending emulator responses back to the child PTY
                // (e.g., DA1 replies, focus event reports).
                // Wrap in catch_unwind because portable-pty internally asserts
                // that writes succeed; when the child has exited and the PTY
                // slave is closed, a write to the master will panic.
                if !responses.is_empty() {
                    let mut output = writer.lock();
                    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        let _ = output.write_all(&responses);
                        let _ = output.flush();
                    }));
                }
                vtty_output.notify_sinks(&snapshot);
            }
        } else {
            // Rate-limited path: use tokio::select! to combine PTY
            // output reception with a periodic flush timer.
            let mut tick = tokio::time::interval(flush_interval);
            tick.tick().await;

            loop {
                tokio::select! {
                    result = pty_out_rx.recv() => {
                        match result {
                            Some(PtyOutput(data)) => {
                                let mut emu = emulator.write().await;
                                emu.feed(&data);
                                let responses = emu.drain_responses();
                                let snapshot = emu.snapshot();
                                drop(emu);
                                // Send any pending emulator responses back
                                // Wrap in catch_unwind for the same reason as the
                                // non-rate-limited path above.
                                if !responses.is_empty() {
                                    let mut output = writer.lock();
                                    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                        let _ = output.write_all(&responses);
                                        let _ = output.flush();
                                    }));
                                }

                                if rate_limiter.allow() {
                                    vtty_output.notify_sinks(&snapshot);
                                    pending_snapshot = None;
                                } else {
                                    pending_snapshot = Some(snapshot);
                                }
                            }
                            None => {
                                if let Some(snapshot) = pending_snapshot.take() {
                                    vtty_output.notify_sinks(&snapshot);
                                }
                                break;
                            }
                        }
                    }
                    _ = tick.tick() => {
                        if let Some(snapshot) = pending_snapshot.take() {
                            vtty_output.notify_sinks(&snapshot);
                        }
                    }
                }
            }
        }
        // PTY closed — flush trailing bytes from parser
        emulator.write().await.finish();
        vtty_output.close();
    });
}

/// Spawn the blocking stdin writer task.
///
/// Receives `StdinMessage` values from the async channel and writes bytes
/// to the PTY master fd.  Signal messages are currently no-ops.
fn spawn_stdin_writer(
    stdin_rx: mpsc::Receiver<StdinMessage>,
    writer: Arc<parking_lot::Mutex<Box<dyn Write + Send>>>,
) {
    tokio::task::spawn_blocking(move || {
        let mut stdin_rx = stdin_rx;
        while let Some(msg) = stdin_rx.blocking_recv() {
            match msg {
                StdinMessage::Bytes(data) => {
                    let _ = writer.lock().write_all(&data);
                    let _ = writer.lock().flush();
                }
                StdinMessage::Signal(_sig) => {}
            }
        }
    });
}

/// Spawn the blocking process waiter task.
///
/// Waits for the child process to exit, then runs exit hooks (per-command
/// and global), saves an optional snapshot, stores exit metadata, and
/// cleans up the command from the manager unless `retain_on_exit` is set.
#[allow(clippy::too_many_arguments)]
fn spawn_process_waiter(
    mut child: crate::process::pty::ChildProcess,
    watch_id: String,
    on_exit: Option<String>,
    on_error: Option<String>,
    global_on_exit: Option<String>,
    global_on_error: Option<String>,
    manager_cmds: Arc<dashmap::DashMap<String, CommandHandle>>,
    logger: Arc<crate::logging::command_log::CommandLogger>,
    snapshot_emulator: Arc<tokio::sync::RwLock<VttyEmulator>>,
    snapshot_on_exit: Option<String>,
    child_exit_tx: tokio::sync::watch::Sender<bool>,
    exit_tx: oneshot::Sender<ExitStatus>,
) {
    tokio::task::spawn_blocking({
        let child_exit_tx = child_exit_tx.clone();
        move || {
            let status = child.wait().ok().flatten();
            let exit_status = ExitStatus {
                code: status,
                signal: None,
            };

            tracing::info!(
                id = %watch_id,
                code = ?exit_status.code,
                "Command exited"
            );

            // Run per-command on_exit or on_error handler if configured
            let per_cmd_hook = if exit_status.success() {
                on_exit.as_ref()
            } else {
                on_error.as_ref()
            };

            if let Some(on_cmd_str) = per_cmd_hook {
                let parts: Vec<&str> = on_cmd_str.split_whitespace().collect();
                if !parts.is_empty() {
                    let binary = parts[0];
                    let cmd_args: Vec<String> = parts[1..].iter().map(|s| s.to_string()).collect();
                    tracing::info!(
                        id = %watch_id,
                        trigger = if exit_status.success() { "on_exit" } else { "on_error" },
                        command = binary,
                        args = ?cmd_args,
                        "Running per-command exit handler"
                    );
                    match std::process::Command::new(binary).args(&cmd_args).spawn() {
                        Ok(mut child) => {
                            let _ = child.try_wait();
                        }
                        Err(e) => {
                            tracing::warn!(
                                id = %watch_id,
                                error = %e,
                                "Failed to run per-command exit handler"
                            );
                        }
                    }
                }
            }

            // Run global on_exit or on_error hook if configured
            let global_hook = if exit_status.success() {
                global_on_exit.as_ref()
            } else {
                global_on_error.as_ref()
            };

            if let Some(global_hook_str) = global_hook {
                let mut vars = HashMap::new();
                vars.insert("name", watch_id.clone());
                vars.insert("id", watch_id.clone());
                vars.insert("pid", 0.to_string());
                vars.insert(
                    "exit_code",
                    exit_status
                        .code
                        .map(|c| c.to_string())
                        .unwrap_or("unknown".to_string()),
                );
                tracing::info!(
                    id = %watch_id,
                    trigger = if exit_status.success() { "global on_exit" } else { "global on_error" },
                    "Running global hook"
                );
                run_hook(global_hook_str, &vars);
            }

            let (cmd_name, cmd_pid) = manager_cmds
                .get(&watch_id)
                .map(|h| (h.name.clone(), h.pid))
                .unwrap_or_else(|| (watch_id.clone(), 0));
            logger.log("exited", &format!("id={} pid={} name={} code={:?}", watch_id, cmd_pid, cmd_name, exit_status.code));

            let _ = child_exit_tx.send(true);

            // Save snapshot to file if snapshot_on_exit is configured.
            // This must happen before the command is removed from the manager
            // (which drops the handle and its emulator).
            if let Some(ref snapshot_path) = snapshot_on_exit {
                // Use block_in_place to safely acquire the async RwLock from
                // a blocking context.
                let snapshot_result = tokio::task::block_in_place(|| {
                    let emu = snapshot_emulator.blocking_read();
                    let buf = emu.snapshot();
                    let mut text = String::new();
                    // Write scrollback lines first
                    for line in &buf.scrollback {
                        let line_str: String = line
                            .iter()
                            .map(|c| if c.width > 0 { c.ch } else { '\0' })
                            .collect();
                        text.push_str(line_str.trim_end());
                        text.push('\n');
                    }
                    // Write visible screen rows
                    for line in &buf.rows {
                        let line_str: String = line
                            .iter()
                            .map(|c| if c.width > 0 { c.ch } else { '\0' })
                            .collect();
                        text.push_str(line_str.trim_end());
                        text.push('\n');
                    }
                    Ok::<String, std::io::Error>(text)
                });

                match snapshot_result {
                    Ok(snapshot_text) => match std::fs::write(snapshot_path, &snapshot_text) {
                        Ok(_) => {
                            tracing::info!(
                                id = %watch_id,
                                path = %snapshot_path,
                                bytes = snapshot_text.len(),
                                "Saved snapshot on exit"
                            );
                        }
                        Err(e) => {
                            tracing::warn!(
                                id = %watch_id,
                                path = %snapshot_path,
                                error = %e,
                                "Failed to save snapshot on exit"
                            );
                        }
                    },
                    Err(e) => {
                        tracing::warn!(
                            id = %watch_id,
                            error = %e,
                            "Failed to acquire emulator for snapshot"
                        );
                    }
                }
            }

            // Store exit metadata in the handle (if still in the manager)
            if let Some(handle) = manager_cmds.get(&watch_id) {
                let code = exit_status.code;
                let now = std::time::Instant::now();
                *handle.exit_code.lock().unwrap() = code;
                *handle.exit_time.lock().unwrap() = Some(now);
                drop(handle); // release DashMap guard
            }

            // Remove from manager unless retain_on_exit is set
            let retain = manager_cmds
                .get(&watch_id)
                .map(|h| h.exit_config.retain_on_exit)
                .unwrap_or(false);
            if retain {
                tracing::info!(id = %watch_id, "Command retained after exit (retain_on_exit)");
                logger.log("exit", &format!("id={} pid={} name={} retained=true code={:?}", watch_id, cmd_pid, cmd_name, exit_status.code));
            } else {
                manager_cmds.remove(&watch_id);
                tracing::info!(id = %watch_id, "Command removed from manager after exit");
                logger.log("exit", &format!("id={} pid={} name={} retained=false code={:?}", watch_id, cmd_pid, cmd_name, exit_status.code));
            }

            let _ = exit_tx.send(exit_status);
        }
    });
}

/// Escape a byte slice into a human-readable string.
///
/// Printable ASCII characters (0x20–0x7E) are passed through as-is.
/// All other bytes (control chars, ESC, high bytes) are represented as
/// `\xHH` hex escapes.  Backslash itself is escaped as `\\` to avoid
/// ambiguity in the log output.
fn escape_bytes(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len() * 2);
    for &b in data {
        match b {
            b'\\' => out.push_str("\\\\"),
            0x20..=0x7e => out.push(b as char),
            _ => write!(out, "\\x{:02x}", b).unwrap(),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_bytes_printable() {
        assert_eq!(escape_bytes(b"Hello World"), "Hello World");
    }

    #[test]
    fn test_escape_bytes_control_chars() {
        assert_eq!(
            escape_bytes(&[0x00, 0x01, 0x1b, 0x7f]),
            "\\x00\\x01\\x1b\\x7f"
        );
    }

    #[test]
    fn test_escape_bytes_backslash() {
        assert_eq!(escape_bytes(b"a\\b"), "a\\\\b");
    }

    #[test]
    fn test_escape_bytes_empty() {
        assert_eq!(escape_bytes(b""), "");
    }

    #[test]
    fn test_exit_status_success() {
        let status = ExitStatus {
            code: Some(0),
            signal: None,
        };
        assert!(status.success());
    }

    #[test]
    fn test_exit_status_error() {
        let status = ExitStatus {
            code: Some(1),
            signal: None,
        };
        assert!(status.is_error());
        assert!(!status.success());
    }

    #[test]
    fn test_exit_status_signal_none() {
        let status = ExitStatus {
            code: None,
            signal: None,
        };
        assert!(!status.success());
    }
}
