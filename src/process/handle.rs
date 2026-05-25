use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, RwLock};
use portable_pty::MasterPty;

use crate::config::schema::ExitConfig;
use crate::vtty::emulator::VttyEmulator;
use crate::vtty::sink::VttyOutput;
use crate::handles::registry::HandleRegistry;
use super::spawner::{StdinMessage, ExitStatus};

pub struct CommandHandle {
    pub id: String,
    pub pid: u32,
    pub name: String,
    pub args: Vec<String>,
    pub emulator: Arc<RwLock<VttyEmulator>>,
    pub stdin_tx: mpsc::Sender<StdinMessage>,
    pub _exit_rx: oneshot::Receiver<ExitStatus>,
    pub handle_registry: HandleRegistry,
    /// Optional certificate name bound to this command.
    pub certificate: Option<String>,
    /// Exit configuration for this command (on_exit, on_error, timeout).
    pub exit_config: ExitConfig,
    /// Wall-clock time when this command was spawned.
    pub spawn_time: std::time::Instant,
    /// PTY master handle for resizing the child PTY (e.g. on SIGWINCH).
    /// Wrapped in a Mutex because `MasterPty` is `Send` but not `Sync`,
    /// which is required by `DashMap` (used in `CommandManager`).
    pub pty_master: Arc<parking_lot::Mutex<Box<dyn MasterPty + Send>>>,
    /// Output sink manager — notified after each emulator feed.
    /// Sinks receive push notifications when the VTTY buffer changes,
    /// replacing the need for polling-based change detection.
    pub vtty_output: Arc<VttyOutput>,
    /// Watch-channel receiver for the child-exit signal.  Unlike
    /// tokio::sync::Notify (which loses notifications when no waiter
    /// is present), a watch channel always stores the latest value.
    /// The display/headless loop awaits `changed()` in select! — it
    /// completes the instant the child exits, even if the child died
    /// before the loop started awaiting.
    ///
    /// Receiver is Clone, so it can be extracted from the DashMap
    /// without needing mutable access.
    pub exit_rx: tokio::sync::watch::Receiver<bool>,
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

    /// Blocking snapshot — used in sync contexts (e.g. inside DashMap iterations).
    /// Uses parking_lot's blocking_read when inside an async context.
    /// Since we're typically called from within a DashMap guard scope,
    /// we use tokio::task::block_in_place to avoid deadlocks.
    pub fn vtty_snapshot_blocking(&self) -> crate::vtty::buffer::Buffer {
        // We are typically already in an async context. Use block_in_place
        // to avoid issues with the tokio runtime.
        let emu = self.emulator.blocking_read();
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

    /// Whether the process is currently using the alternate screen buffer.
    pub async fn is_alternate_screen(&self) -> bool {
        let emu = self.emulator.read().await;
        emu.is_alternate_screen()
    }

    /// Get the main buffer as HTML (even if alt screen is active).
    pub async fn vtty_html_main(&self) -> String {
        let emu = self.emulator.read().await;
        let buf = emu.snapshot_main();
        crate::vtty::renderer::VttyRenderer::to_html(&buf)
    }

    /// Get the alternate buffer as HTML (or empty if never used).
    pub async fn vtty_html_alt(&self) -> String {
        let emu = self.emulator.read().await;
        let buf = emu.snapshot_alt();
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

    /// Resize both the VTTY emulator AND the underlying child PTY.
    /// This is the correct way to handle terminal resize (SIGWINCH):
    ///   1. Resize the PTY master → kernel sends SIGWINCH to the child
    ///   2. Resize the in-memory VTTY buffer to match
    pub async fn resize_pty(&self, rows: u16, cols: u16) -> anyhow::Result<()> {
        // Resize the PTY master first — this sends SIGWINCH to the child.
        // The Mutex lock is very short-lived (just an ioctl syscall).
        {
            let master = self.pty_master.lock();
            master.resize(portable_pty::PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            }).map_err(|e| anyhow::anyhow!("PTY resize failed: {}", e))?;
        }

        // Then resize the in-memory VTTY buffer
        let mut emu = self.emulator.write().await;
        emu.resize(rows as usize, cols as usize);

        Ok(())
    }

    pub fn list_handles(&self) -> Vec<String> {
        self.handle_registry.list()
    }

    /// Elapsed wall-clock time since spawn.
    pub fn runtime_secs(&self) -> f64 {
        self.spawn_time.elapsed().as_secs_f64()
    }

    /// Whether the underlying OS process is still alive (pid check).
    pub fn is_alive(&self) -> bool {
        #[cfg(unix)]
        {
            unsafe { libc::kill(self.pid as i32, 0) == 0 }
        }
        #[cfg(not(unix))]
        {
            true // best-effort on non-Unix
        }
    }
}
