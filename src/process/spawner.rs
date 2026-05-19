use anyhow::Result;
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;
use std::sync::Arc;

use crate::config::schema::VttyConfig;
use crate::vtty::emulator::VttyEmulator;
use super::handle::CommandHandle;

/// A running process with its PTY, VTTY emulator, and communication channels.
pub struct ProcessSpawner {
    vtty_cfg: VttyConfig,
}

/// Messages that can be sent to the process's stdin.
pub enum StdinMessage {
    /// Raw bytes to write
    Bytes(Vec<u8>),
    /// Signal to send (e.g., SIGINT)
    Signal(String),
}

impl ProcessSpawner {
    pub fn new(vtty_cfg: &VttyConfig) -> Self {
        Self {
            vtty_cfg: vtty_cfg.clone(),
        }
    }

    /// Spawn a new process in a PTY and return a handle to control it.
    pub async fn spawn(
        &self,
        cmd: String,
        args: Vec<String>,
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

        let child = pair.slave.spawn_command(cmd_builder)?;
        let pid = child.process_id().unwrap_or(0);

        // Create the VTTY emulator
        let emulator = Arc::new(tokio::sync::RwLock::new(VttyEmulator::new(
            self.vtty_cfg.rows,
            self.vtty_cfg.cols,
            self.vtty_cfg.scrollback,
        )));

        // Channel for stdin injection
        let (stdin_tx, mut stdin_rx) = mpsc::channel::<StdinMessage>(128);

        // Get the PTY master for reading/writing
        let mut reader = pair.master.try_clone_reader()?;
        let mut writer = pair.master.try_clone_writer()?;

        // Spawn PTY reader task: reads from PTY → feeds VTTY emulator
        let emu_for_reader = emulator.clone();
        tokio::spawn(async move {
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf).await {
                    Ok(0) => break, // EOF
                    Ok(n) => {
                        let data = buf[..n].to_vec();
                        let mut emu = emu_for_reader.write().await;
                        emu.feed(&data);
                    }
                    Err(_) => break,
                }
            }
        });

        // Spawn stdin writer task: receives from channel → writes to PTY
        tokio::spawn(async move {
            while let Some(msg) = stdin_rx.recv().await {
                match msg {
                    StdinMessage::Bytes(data) => {
                        let _ = writer.write_all(&data).await;
                        let _ = writer.flush().await;
                    }
                    StdinMessage::Signal(_sig) => {
                        // Signal handling would require platform-specific code
                        // For now, we handle common signals via byte sequences
                    }
                }
            }
        });

        // Spawn process waiter task
        let (exit_tx, exit_rx) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            let _ = child.wait();
            let _ = exit_tx.send(());
        });

        Ok(CommandHandle {
            id: String::new(), // set by manager
            pid,
            name: cmd,
            emulator,
            stdin_tx,
            _exit_rx: exit_rx,
        })
    }
}
