use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, RwLock};

use crate::vtty::emulator::VttyEmulator;
use super::spawner::StdinMessage;

/// Handle to a running command with its VTTY and communication channels.
pub struct CommandHandle {
    pub id: String,
    pub pid: u32,
    pub name: String,
    /// The VTTY emulator for this process (shared for concurrent reads)
    pub emulator: Arc<RwLock<VttyEmulator>>,
    /// Channel for sending stdin to the process
    pub stdin_tx: mpsc::Sender<StdinMessage>,
    /// Receiver for process exit notification
    pub _exit_rx: oneshot::Receiver<()>,
}

impl CommandHandle {
    /// Send raw bytes to the process's stdin.
    pub async fn send_bytes(&self, data: Vec<u8>) -> anyhow::Result<()> {
        self.stdin_tx.send(StdinMessage::Bytes(data)).await
            .map_err(|_| anyhow::anyhow!("stdin channel closed"))
    }

    /// Send a signal-like byte sequence to the process.
    pub async fn send_signal(&self, signal: String) -> anyhow::Result<()> {
        self.stdin_tx.send(StdinMessage::Signal(signal)).await
            .map_err(|_| anyhow::anyhow!("stdin channel closed"))
    }

    /// Kill the process (best effort via signal bytes).
    pub async fn kill(&self) -> anyhow::Result<()> {
        // Send Ctrl+C (SIGINT equivalent)
        self.send_bytes(vec![0x03]).await?;
        Ok(())
    }

    /// Get a snapshot of the VTTY buffer.
    pub async fn vtty_snapshot(&self) -> crate::vtty::buffer::Buffer {
        let emu = self.emulator.read().await;
        emu.snapshot()
    }

    /// Get plain text contents of the VTTY.
    pub async fn vtty_plain(&self) -> String {
        let emu = self.emulator.read().await;
        emu.contents_plain()
    }

    /// Get ANSI-encoded contents of the VTTY.
    pub async fn vtty_ansi(&self) -> String {
        let emu = self.emulator.read().await;
        emu.contents_ansi()
    }

    /// Get partial VTTY contents.
    pub async fn vtty_partial(&self, start_row: usize, row_count: usize) -> String {
        let emu = self.emulator.read().await;
        emu.partial(start_row, row_count)
    }
}
