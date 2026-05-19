use std::io::{Read as _, Write as _};
use anyhow::Result;
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use tokio::sync::mpsc;
use std::sync::Arc;

use crate::config::schema::{VttyConfig, HandleConfig};
use crate::handles::{
    file_sink::FileSink,
    null_sink::NullSink,
    registry::HandleRegistry,
    sink::Sink,
    vtty_sink::VttySink,
};
use crate::vtty::emulator::VttyEmulator;
use super::handle::CommandHandle;

pub struct ProcessSpawner {
    vtty_cfg: VttyConfig,
}

pub enum StdinMessage {
    Bytes(Vec<u8>),
    Signal(String),
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

        // Channel for stdin injection
        let (stdin_tx, mut stdin_rx) = mpsc::channel::<StdinMessage>(128);

        // Get PTY master reader and writer (both are synchronous I/O)
        let mut reader = pair.master.try_clone_reader()?;
        let mut writer = pair.master.take_writer()?;

        // Spawn PTY reader task in a blocking thread (portable-pty uses sync I/O)
        let emu_for_reader = emulator.clone();
        tokio::task::spawn_blocking(move || {
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        let data = buf[..n].to_vec();
                        // We need the tokio runtime to write to the emulator's RwLock.
                        // Use try_write to avoid deadlocking, and re-queue on contention.
                        let emu = emu_for_reader.clone();
                        tokio::spawn(async move {
                            let mut emu = emu.write().await;
                            emu.feed(&data);
                        });
                    }
                    Err(_) => break,
                }
            }
        });

        // Spawn stdin writer task in a blocking thread (portable-pty uses sync I/O)
        tokio::task::spawn_blocking(move || {
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

        // Spawn process waiter
        let (exit_tx, exit_rx) = tokio::sync::oneshot::channel();
        tokio::task::spawn_blocking(move || {
            let _ = child.wait();
            let _ = exit_tx.send(());
        });

        Ok(CommandHandle {
            id: command_id.to_string(),
            pid,
            name: cmd,
            emulator,
            stdin_tx,
            _exit_rx: exit_rx,
            handle_registry,
        })
    }
}
