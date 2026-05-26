use std::io::{Read as _, Write as _};
use std::fmt::Write as FmtWrite;

use super::error::Result;
use super::pty::{PtyBackend, PortablePtyBackend, PtySize};
use tokio::sync::{mpsc, oneshot};
use std::sync::Arc;

use std::collections::HashMap;

use crate::config::schema::{VttyConfig, HandleConfig, ExitConfig, RateLimitConfig};
use crate::config::hooks::HooksConfig;
use crate::hooks::runner::run_hook;
use crate::handles::{
    file_sink::FileSink,
    null_sink::NullSink,
    registry::HandleRegistry,
    sink::Sink,
    vtty_sink::VttySink,
};
use crate::vtty::buffer::Buffer;
use crate::vtty::emulator::VttyEmulator;
use crate::vtty::rate_limiter::RateLimiter;
use crate::vtty::sink::{VttyOutput, BroadcastVttySink};
use crate::process::manager::CommandManager;
use super::handle::CommandHandle;

pub struct ProcessSpawner {
    vtty_cfg: VttyConfig,
    rate_limit_cfg: RateLimitConfig,
    /// The PTY backend used to open pseudo-terminals.
    /// Defaults to [`PortablePtyBackend`] but can be swapped for testing
    /// or to use a custom implementation (Unix native PTY, ConPTY, etc.).
    pty_backend: Box<dyn PtyBackend>,
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
    pub fn new(vtty_cfg: &VttyConfig, rate_limit_cfg: &RateLimitConfig) -> Self {
        Self {
            vtty_cfg: vtty_cfg.clone(),
            rate_limit_cfg: rate_limit_cfg.clone(),
            pty_backend: Box::new(PortablePtyBackend::new()),
        }
    }

    /// Create a spawner with a custom PTY backend.
    ///
    /// This allows injecting alternative PTY implementations for testing
    /// or platform-specific backends (Unix native PTY, ConPTY, etc.).
    pub fn with_backend(
        vtty_cfg: &VttyConfig,
        rate_limit_cfg: &RateLimitConfig,
        backend: Box<dyn PtyBackend>,
    ) -> Self {
        Self {
            vtty_cfg: vtty_cfg.clone(),
            rate_limit_cfg: rate_limit_cfg.clone(),
            pty_backend: backend,
        }
    }

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
        pty_raw_log: Option<&str>,
    ) -> Result<CommandHandle> {
        let _cmd_display = cmd.clone(); // for error reporting
        // Use per-command overrides if provided, otherwise fall back to config defaults
        let rows = rows.unwrap_or(self.vtty_cfg.rows);
        let cols = cols.unwrap_or(self.vtty_cfg.cols);

        let pair = self.pty_backend.openpty(PtySize { rows, cols })?;

        // Spawn the child process via the PTY slave
        let mut child = pair.slave.spawn_command(
            &cmd,
            &args,
            &self.vtty_cfg.term,
            &env_vars,
        )?;
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

        // Create handle registry and wire sinks
        let mut handle_registry = HandleRegistry::new();
        for cfg in handle_configs {
            let sink: Box<dyn Sink> = match cfg.sink.as_str() {
                "file" => {
                    let path = cfg.path.as_deref().unwrap_or("/dev/null");
                    // Substitute placeholders
                    let path = path.replace("{id}", command_id).replace("{name}", &cmd);
                    Box::new(FileSink::new(&path)?)
                }
                "vtty" => Box::new(VttySink::new()),
                "null" => Box::new(NullSink),
                _ => Box::new(NullSink),
            };
            handle_registry.add(cfg.name, sink);
        }

        // Channel for stdin injection (async → blocking)
        let (stdin_tx, mut stdin_rx) = mpsc::channel::<StdinMessage>(128);

        // Channel for PTY output (blocking → async)
        // Uses a bounded channel to provide backpressure: if the async consumer
        // (emulator writer) is slow, the blocking reader will naturally wait.
        let (pty_out_tx, mut pty_out_rx) = mpsc::channel::<PtyOutput>(64);

        // Get PTY master reader and writer
        let reader = pair.master.clone_reader()?;
        let writer = pair.master.take_writer()?;

        // Store the PTY master handle for later resize (e.g. WINCH handling).
        let pty_master: Arc<parking_lot::Mutex<Box<dyn crate::process::pty::PtyMaster + Send>>> =
            Arc::new(parking_lot::Mutex::new(pair.master));

        // Spawn PTY reader task in a blocking thread.
        //
        // When pty_raw_log is set, raw bytes from each read() call are
        // logged to the specified file in a human-readable escaped format
        // (printable ASCII as-is, non-printable as \xHH) with an elapsed-
        // time stamp.  This log can be replayed with the ansi-replay tool.
        let pty_raw_log_owned = pty_raw_log.map(|s| s.to_string());
        tokio::task::spawn_blocking(move || {
            let mut reader = reader;
            let mut buf = [0u8; 4096];
            let start = std::time::Instant::now();

            // Open the raw PTY log file if configured.
            let mut log_file: Option<std::fs::File> = pty_raw_log_owned.as_deref().and_then(|path| {
                match std::fs::File::create(path) {
                    Ok(f) => {
                        tracing::info!(path = path, "PTY raw log opened");
                        Some(f)
                    }
                    Err(e) => {
                        tracing::warn!(path = path, error = %e,
                            "Failed to open PTY raw log, skipping");
                        None
                    }
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

        // Create VttyOutput with a BroadcastVttySink that pushes dirty
        // notifications via the same broadcast channel used by the existing
        // diff watcher.  This provides an immediate, push-based notification
        // path that complements the polling-based watcher.
        let vtty_output: Arc<VttyOutput> = Arc::new(VttyOutput::with_sinks(vec![
            Arc::new(BroadcastVttySink::new(
                manager.vtty_change_sender(),
                command_id.to_string(),
            )),
        ]));

        // Wrap PTY writer in Arc<Mutex> for shared access between the stdin
        // writer task and the PTY output consumer (which needs to write
        // emulator responses like DA1 replies back to the child PTY).
        let writer = Arc::new(parking_lot::Mutex::new(writer));

        // Spawn async PTY output consumer — feeds data into the emulator
        // and notifies all registered VttySinks, with rate limiting.
        let emu_writer = emulator.clone();
        let sink_output = vtty_output.clone();
        let emu_writer_ptm = writer.clone();
        let mut rate_limiter = RateLimiter::from_config(self.rate_limit_cfg.max_updates_per_sec);
        let flush_interval = rate_limiter.interval();
        tokio::spawn(async move {
            // State for rate-limited buffering.
            let mut pending_snapshot: Option<Buffer> = None;
            // If rate limiting is disabled, use a simple loop (no timer overhead).
            if rate_limiter.is_disabled() {
                while let Some(PtyOutput(data)) = pty_out_rx.recv().await {
                    let mut emu = emu_writer.write().await;
                    emu.feed(&data);
                    let responses = emu.drain_responses();
                    let snapshot = emu.snapshot();
                    drop(emu);
                    // Send any pending emulator responses back to the child PTY
                    // (e.g., DA1 replies, focus event reports).
                    if !responses.is_empty() {
                        let _ = emu_writer_ptm.lock().write_all(&responses);
                        let _ = emu_writer_ptm.lock().flush();
                    }
                    sink_output.notify_sinks(&snapshot);
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
                                    let mut emu = emu_writer.write().await;
                                    emu.feed(&data);
                                    let responses = emu.drain_responses();
                                    let snapshot = emu.snapshot();
                                    drop(emu);
                                    // Send any pending emulator responses back
                                    if !responses.is_empty() {
                                        let _ = emu_writer_ptm.lock().write_all(&responses);
                                        let _ = emu_writer_ptm.lock().flush();
                                    }

                                    if rate_limiter.allow() {
                                        sink_output.notify_sinks(&snapshot);
                                        pending_snapshot = None;
                                    } else {
                                        pending_snapshot = Some(snapshot);
                                    }
                                }
                                None => {
                                    if let Some(snapshot) = pending_snapshot.take() {
                                        sink_output.notify_sinks(&snapshot);
                                    }
                                    break;
                                }
                            }
                        }
                        _ = tick.tick() => {
                            if let Some(snapshot) = pending_snapshot.take() {
                                sink_output.notify_sinks(&snapshot);
                            }
                        }
                    }
                }
            }
            // PTY closed — flush trailing bytes from parser
            emu_writer.write().await.finish();
            sink_output.close();
        });

        // Spawn stdin writer task in a blocking thread.
        // Shares the PTY writer with the emulator output consumer via Arc<Mutex>.
        let stdin_writer = writer.clone();
        tokio::task::spawn_blocking(move || {
            while let Some(msg) = stdin_rx.blocking_recv() {
                match msg {
                    StdinMessage::Bytes(data) => {
                        let _ = stdin_writer.lock().write_all(&data);
                        let _ = stdin_writer.lock().flush();
                    }
                    StdinMessage::Signal(_sig) => {}
                }
            }
        });

        // Spawn process waiter (blocking — child.wait() is sync)
        let (exit_tx, exit_rx) = oneshot::channel::<ExitStatus>();
        let watch_id = command_id.to_string();
        let on_exit = exit_config.on_exit.clone();
        let on_error = exit_config.on_error.clone();
        let global_on_exit = hooks.on_exit.clone();
        let global_on_error = hooks.on_error.clone();
        let manager_cmds = manager.commands_arc();
        let (child_exit_tx, child_exit_rx) = tokio::sync::watch::channel(false);
        let snapshot_on_exit = exit_config.snapshot_on_exit.clone();
        let snapshot_emulator = emulator.clone();

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

            if let Some(ref on_cmd_str) = per_cmd_hook {
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

            if let Some(ref global_hook_str) = global_hook {
                let mut vars = HashMap::new();
                vars.insert("name", watch_id.clone());
                vars.insert("id", watch_id.clone());
                vars.insert("pid", 0.to_string());
                vars.insert("exit_code", exit_status.code.map(|c| c.to_string()).unwrap_or("unknown".to_string()));
                tracing::info!(
                    id = %watch_id,
                    trigger = if exit_status.success() { "global on_exit" } else { "global on_error" },
                    "Running global hook"
                );
                run_hook(global_hook_str, &vars);
            }

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
                        let line_str: String = line.iter()
                            .map(|c| if c.width > 0 { c.ch } else { '\0' })
                            .collect();
                        text.push_str(line_str.trim_end());
                        text.push('\n');
                    }
                    // Write visible screen rows
                    for line in &buf.rows {
                        let line_str: String = line.iter()
                            .map(|c| if c.width > 0 { c.ch } else { '\0' })
                            .collect();
                        text.push_str(line_str.trim_end());
                        text.push('\n');
                    }
                    Ok::<String, std::io::Error>(text)
                });

                match snapshot_result {
                    Ok(snapshot_text) => {
                        match std::fs::write(snapshot_path, &snapshot_text) {
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
                        }
                    }
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
            let retain = manager_cmds.get(&watch_id)
                .map(|h| h.exit_config.retain_on_exit)
                .unwrap_or(false);
            if retain {
                tracing::info!(id = %watch_id, "Command retained after exit (retain_on_exit)");
            } else {
                manager_cmds.remove(&watch_id);
                tracing::info!(id = %watch_id, "Command removed from manager after exit");
            }

            let _ = exit_tx.send(exit_status);
        }
        });

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
            pty_master,
            vtty_output,
            exit_rx: child_exit_rx,
            exit_code: std::sync::Mutex::new(None),
            exit_time: std::sync::Mutex::new(None),
        })
    }
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
