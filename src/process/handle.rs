use crate::process::pty::PtyMaster;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, RwLock};

use super::error::{ProcessError, Result};

use super::spawner::{ExitStatus, StdinMessage};
use crate::config::schema::ExitConfig;
use crate::handles::registry::HandleRegistry;
use crate::vtty::emulator::VttyEmulator;
use crate::vtty::sink::VttyOutput;

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
    /// Wrapped in a Mutex because `PtyMaster` methods may not be thread-safe.
    /// Uses the `PtyMaster` trait for backend independence.
    pub pty_master: Arc<parking_lot::Mutex<Box<dyn PtyMaster + Send>>>,
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
    /// Exit code of the child process (set when the child terminates).
    /// None while the process is still running.
    pub exit_code: std::sync::Mutex<Option<i32>>,
    /// Wall-clock time when the child process exited.
    /// None while the process is still running.
    pub exit_time: std::sync::Mutex<Option<std::time::Instant>>,
    /// Whether the process has been frozen (SIGSTOP).  Uses AtomicBool
    /// so any thread can check or flip the flag without holding a lock.
    pub frozen: std::sync::atomic::AtomicBool,
}

impl CommandHandle {
    pub async fn send_bytes(&self, data: Vec<u8>) -> Result<()> {
        self.stdin_tx
            .send(StdinMessage::Bytes(data))
            .await
            .map_err(|_| ProcessError::ChannelClosed(self.id.clone()))
    }

    pub async fn send_signal(&self, signal: String) -> Result<()> {
        self.stdin_tx
            .send(StdinMessage::Signal(signal))
            .await
            .map_err(|_| ProcessError::ChannelClosed(self.id.clone()))
    }

    pub async fn kill(&self) -> Result<()> {
        self.send_bytes(vec![0x03]).await?;
        Ok(())
    }

    pub async fn vtty_snapshot(&self) -> crate::vtty::buffer::Buffer {
        let emu = self.emulator.read().await;
        emu.snapshot()
    }

    /// Blocking snapshot — used in sync contexts (e.g. inside DashMap iterations,
    /// display rendering) that are called from within the async runtime.
    ///
    /// Uses `tokio::task::block_in_place` to safely call `blocking_read()` from
    /// an async context. Without this wrapper, `blocking_read()` on a
    /// `tokio::sync::RwLock` panics with "Cannot block the current thread from
    /// within a runtime".
    pub fn vtty_snapshot_blocking(&self) -> crate::vtty::buffer::Buffer {
        tokio::task::block_in_place(|| {
            let emu = self.emulator.blocking_read();
            emu.snapshot()
        })
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

    /// Get the VTTY buffer as HTML including scrollback lines.
    ///
    /// `scrollback_offset` shifts the viewport backward (0 = normal bottom).
    /// `visible_rows` is how many rows of HTML to return.
    pub async fn vtty_html_scrollback(
        &self,
        scrollback_offset: usize,
        visible_rows: usize,
    ) -> String {
        let buf = self.vtty_snapshot().await;
        crate::vtty::renderer::VttyRenderer::to_html_scrollback(
            &buf,
            scrollback_offset,
            visible_rows,
        )
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

    /// Whether the child application has made the cursor visible (DEC mode 25).
    pub async fn is_cursor_visible(&self) -> bool {
        let emu = self.emulator.read().await;
        emu.is_cursor_visible()
    }

    /// Get the current cursor style.
    pub async fn cursor_style(&self) -> crate::vtty::emulator::CursorStyle {
        let emu = self.emulator.read().await;
        emu.cursor_style()
    }

    /// Send pasted text to the command, wrapping in bracketed paste
    /// escape sequences if the child has enabled bracketed paste mode (?2004).
    /// When bracketed paste mode is active, the text is sent as:
    ///   ESC[200~ text ESC[201~
    /// This allows the child application to distinguish pasted text from
    /// typed input (e.g., to avoid triggering auto-complete or line editing).
    pub async fn send_paste(&self, text: &str) -> Result<()> {
        let bracketed = {
            let emu = self.emulator.read().await;
            emu.bracketed_paste_enabled()
        };
        if bracketed {
            let mut data = b"\x1b[200~".to_vec();
            data.extend_from_slice(text.as_bytes());
            data.extend_from_slice(b"\x1b[201~");
            self.send_bytes(data).await
        } else {
            self.send_bytes(text.as_bytes().to_vec()).await
        }
    }

    pub async fn dimensions(&self) -> (usize, usize) {
        let emu = self.emulator.read().await;
        emu.dimensions()
    }

    pub async fn scrollback_count(&self) -> usize {
        let emu = self.emulator.read().await;
        emu.snapshot().scrollback.len()
    }

    /// Whether any mouse tracking mode is enabled (1002/1003).
    pub async fn mouse_tracking_enabled(&self) -> bool {
        let emu = self.emulator.read().await;
        emu.mouse_tracking_enabled()
    }

    /// Whether SGR extended mouse coordinates (?1006) is enabled.
    pub async fn mouse_sgr_enabled(&self) -> bool {
        let emu = self.emulator.read().await;
        emu.mouse_sgr_enabled()
    }

    pub async fn resize(&self, rows: u16, cols: u16) -> Result<()> {
        let mut emu = self.emulator.write().await;
        emu.resize(rows as usize, cols as usize);
        Ok(())
    }

    /// Resize both the VTTY emulator AND the underlying child PTY.
    /// This is the correct way to handle terminal resize (SIGWINCH):
    ///   1. Resize the PTY master → kernel sends SIGWINCH to the child
    ///   2. Resize the in-memory VTTY buffer to match
    pub async fn resize_pty(&self, rows: u16, cols: u16) -> Result<()> {
        // Resize the PTY master first — this sends SIGWINCH to the child.
        {
            let master = self.pty_master.lock();
            master.resize(rows, cols)?;
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

    /// Whether the process has been frozen (SIGSTOP / suspended).
    pub fn is_frozen(&self) -> bool {
        self.frozen.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Whether the child has enabled focus reporting (?1004h).
    pub async fn focus_reporting_enabled(&self) -> bool {
        let emu = self.emulator.read().await;
        emu.focus_reporting_enabled()
    }

    /// Force-exit the alternate screen if active (auto-recovery).
    /// Called when a command exits without properly restoring the main screen.
    pub async fn recover_alternate_screen(&self) {
        let mut emu = self.emulator.write().await;
        emu.recover_from_alternate_screen();
    }
}
