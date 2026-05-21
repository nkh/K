use std::io::{Read as _, Write as _};
use anyhow::Result;
use portable_pty::{CommandBuilder, MasterPty, PtySize, native_pty_system};
use tokio::sync::{mpsc, oneshot};
use std::sync::Arc;

use std::collections::HashMap;

use crate::config::schema::{VttyConfig, HandleConfig, ExitConfig};
use crate::handles::{
    file_sink::FileSink,
    null_sink::NullSink,
    registry::HandleRegistry,
    sink::Sink,
    vtty_sink::VttySink,
};
use crate::vtty::emulator::VttyEmulator;
use crate::process::manager::CommandManager;
use super::handle::CommandHandle;

pub struct ProcessSpawner {
    vtty_cfg: VttyConfig,
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
        }
    }

    pub async fn spawn(
        &self,
        cmd: String,
        args: Vec<String>,
        handle_configs: Vec<HandleConfig>,
        command_id: &str,
        exit_config: ExitConfig,
        env_vars: HashMap<String, String>,
        manager: &CommandManager,
    ) -> Result<CommandHandle> {
        let pty_system = native_pty_system();
        let pair = pty_system.openpty(PtySize {
            rows: self.vtty_cfg.rows,
            cols: self.vtty_cfg.cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let mut cmd_builder = CommandBuilder::new(&cmd);
        for arg in &args {
            cmd_builder.arg(arg);
        }

        // Apply environment variables.
        // portable_pty inherits the parent environment by default.
        // We clear it and set only the explicitly provided vars so that
        // per-command env isolation is controlled and predictable.
        if !env_vars.is_empty() {
            cmd_builder.env("TERM", &self.vtty_cfg.term);
            for (key, value) in &env_vars {
                cmd_builder.env(key, value);
            }
        } else {
            // No explicit env vars — still ensure TERM is set correctly
            cmd_builder.env("TERM", &self.vtty_cfg.term);
        }

        let mut child = pair.slave.spawn_command(cmd_builder)?;
        let pid = child.process_id().unwrap_or(0);

        // Create VTTY emulator
        let emulator = Arc::new(tokio::sync::RwLock::new(VttyEmulator::new(
            self.vtty_cfg.rows,
            self.vtty_cfg.cols,
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

        // Get PTY master reader and writer (both are synchronous I/O from portable-pty)
        let reader = pair.master.try_clone_reader()?;
        let writer = pair.master.take_writer()?;
        // Store the PTY master handle for later resize (e.g. WINCH handling).
        // All MasterPty methods take &self, so it remains valid after
        // extracting reader/writer.
        let pty_master: Arc<parking_lot::Mutex<Box<dyn MasterPty + Send>>> =
            Arc::new(parking_lot::Mutex::new(pair.master));

        // Spawn PTY reader task in a blocking thread.
        // portable-pty returns std::io::Read implementations, not async.
        // We use blocking_send to bridge data to the async world via mpsc.
        tokio::task::spawn_blocking(move || {
            let mut reader = reader;
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break, // EOF — child process closed
                    Ok(n) => {
                        let data = PtyOutput(buf[..n].to_vec());
                        // blocking_send will wait if the channel is full,
                        // providing natural backpressure against fast PTY output.
                        if pty_out_tx.blocking_send(data).is_err() {
                            // Receiver dropped — emulator task gone, shut down reader
                            break;
                        }
                    }
                    Err(_) => break, // PTY read error
                }
            }
        });

        // Spawn async PTY output consumer — feeds data into the emulator.
        // This is the single async task that writes to the emulator, avoiding
        // the previous pattern of spawning a new task per 4KB chunk.
        let emu_writer = emulator.clone();
        tokio::spawn(async move {
            while let Some(PtyOutput(data)) = pty_out_rx.recv().await {
                let mut emu = emu_writer.write().await;
                emu.feed(&data);
            }
        });

        // Spawn stdin writer task in a blocking thread.
        // Uses blocking_recv to bridge from the async mpsc channel to sync I/O.
        tokio::task::spawn_blocking(move || {
            let mut writer = writer;
            while let Some(msg) = stdin_rx.blocking_recv() {
                match msg {
                    StdinMessage::Bytes(data) => {
                        let _ = writer.write_all(&data);
                        let _ = writer.flush();
                    }
                    StdinMessage::Signal(_sig) => {}
                }
            }
        });

        // Spawn process waiter (blocking — child.wait() is sync)
        //
        // After the child exits, this task:
        //   1. Runs on_exit/on_error handler if configured
        //   2. Removes the command from the CommandManager's DashMap
        //   3. Sends the exit status via the oneshot channel
        //
        // Removing the command from the DashMap is critical for the
        // display loop to detect that the command has exited (via
        // manager.get(id) returning None).
        let (exit_tx, exit_rx) = oneshot::channel::<ExitStatus>();
        let watch_id = command_id.to_string();
        let on_exit = exit_config.on_exit.clone();
        let on_error = exit_config.on_error.clone();
        let manager_cmds = manager.commands_arc();

        tokio::task::spawn_blocking(move || {
            let status = child.wait().ok().and_then(|s| Some(s.exit_code() as i32));
            let exit_status = ExitStatus {
                code: status,
                signal: None,
            };

            // Log the exit
            tracing::info!(
                id = %watch_id,
                code = ?exit_status.code,
                "Command exited"
            );

            // Run on_exit or on_error command if configured
            let on_cmd = if exit_status.success() {
                on_exit.as_ref()
            } else {
                on_error.as_ref()
            };

            if let Some(ref on_cmd_str) = on_cmd {
                // Parse command: split on whitespace
                let parts: Vec<&str> = on_cmd_str.split_whitespace().collect();
                if !parts.is_empty() {
                    let binary = parts[0];
                    let cmd_args: Vec<String> = parts[1..].iter().map(|s| s.to_string()).collect();
                    tracing::info!(
                        id = %watch_id,
                        trigger = if exit_status.success() { "on_exit" } else { "on_error" },
                        command = binary,
                        args = ?cmd_args,
                        "Running exit handler"
                    );
                    // Run the exit handler as a detached process (fire and forget)
                    match std::process::Command::new(binary).args(&cmd_args).spawn() {
                        Ok(mut child) => {
                            // Don't wait for it — fire and forget
                            let _ = child.try_wait();
                        }
                        Err(e) => {
                            tracing::warn!(
                                id = %watch_id,
                                error = %e,
                                "Failed to run exit handler"
                            );
                        }
                    }
                }
            }

            // Remove the command from the manager's DashMap.
            // This signals to the display loop (and diff watcher)
            // that the command has exited.
            manager_cmds.remove(&watch_id);
            tracing::info!(id = %watch_id, "Command removed from manager after exit");

            // Send exit status to anyone listening
            let _ = exit_tx.send(exit_status);
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
        })
    }
}
