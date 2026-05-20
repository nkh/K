use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, RwLock};

use crate::vtty::emulator::VttyEmulator;
use crate::handles::registry::HandleRegistry;
use super::spawner::StdinMessage;

pub struct CommandHandle {
    pub id: String,
    pub pid: u32,
    pub name: String,
    pub emulator: Arc<RwLock<VttyEmulator>>,
    pub stdin_tx: mpsc::Sender<StdinMessage>,
    pub _exit_rx: oneshot::Receiver<()>,
    pub handle_registry: HandleRegistry,
    /// Optional certificate name bound to this command.
    /// When set, only clients presenting this certificate (or its derived token)
    /// can interact with the command's endpoints.
    pub certificate: Option<String>,
}

impl CommandHandle {
    pub async fn send_bytes(&self, data: Vec<u8>) -> anyhow::Result<()> {
        self.stdin_tx.send(StdinMessage::Bytes(data)).await
            .map_err(|_| anyhow::anyhow!("stdin channel closed"))
    }

    pub async fn send_signal(&self, signal: String) -> anyhow::Result<()> {
        self.stdin_tx.send(StdinMessage::Signal(signal)).await
            .map_err(|_| anyhow::anyhow!("stdin channel closed"))
    }

    pub async fn kill(&self) -> anyhow::Result<()> {
        self.send_bytes(vec![0x03]).await?;
        Ok(())
    }

    pub async fn vtty_snapshot(&self) -> crate::vtty::buffer::Buffer {
        let emu = self.emulator.read().await;
        emu.snapshot()
    }

    pub async fn vtty_plain(&self) -> String {
        let emu = self.emulator.read().await;
        emu.contents_plain()
    }

    pub async fn vtty_ansi(&self) -> String {
        let emu = self.emulator.read().await;
        emu.contents_ansi()
    }

    pub async fn vtty_partial(&self, start_row: usize, row_count: usize) -> String {
        let emu = self.emulator.read().await;
        emu.partial(start_row, row_count)
    }

    pub async fn vtty_html(&self) -> String {
        let buf = self.vtty_snapshot().await;
        crate::vtty::renderer::VttyRenderer::to_html(&buf)
    }

    pub async fn cursor_position(&self) -> (usize, usize) {
        let emu = self.emulator.read().await;
        emu.cursor()
    }

    pub async fn dimensions(&self) -> (usize, usize) {
        let emu = self.emulator.read().await;
        emu.dimensions()
    }

    pub async fn scrollback_count(&self) -> usize {
        let emu = self.emulator.read().await;
        emu.snapshot().scrollback.len()
    }

    pub async fn resize(&self, rows: u16, cols: u16) -> anyhow::Result<()> {
        let mut emu = self.emulator.write().await;
        emu.resize(rows as usize, cols as usize);
        Ok(())
    }

    pub fn list_handles(&self) -> Vec<String> {
        self.handle_registry.list()
    }
}
