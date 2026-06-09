use crate::process::pty::PtyMaster;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, Mutex, RwLock};

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
    /// Previous buffer snapshot used for computing incremental diffs (Level 3).
    /// Stored per-handle so the HTTP diff endpoint can compute diffs without
    /// relying on the WS diff watcher's local state.
    pub prev_diff_snapshot: Mutex<Option<crate::vtty::buffer::Buffer>>,
}

/// VTTY metadata bundle — all terminal state in one struct.
/// Produced by `CommandHandle::vtty_metadata()` which acquires the read lock once.
pub struct VttyMetadata {
    pub cursor: (usize, usize),
    pub dimensions: (usize, usize),
    pub scrollback_lines: usize,
    pub alternate_screen: bool,
    pub cursor_visible: bool,
    pub mouse_tracking: bool,
    pub mouse_sgr: bool,
    pub generation: u64,
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

    /// Render VTTY buffer as HTML with run-length encoding.
    /// Acquires the read lock once, snapshots, and renders.
    pub async fn vtty_html(&self) -> String {
        let emu = self.emulator.read().await;
        let buf = emu.snapshot();
        crate::vtty::renderer::VttyRenderer::to_html(&buf)
    }

    /// Render VTTY buffer as a PNG image (vrw only).
    #[cfg(feature = "vrw")]
    pub async fn vtty_png(&self, font_size: f32, font_path: Option<&str>) -> anyhow::Result<Vec<u8>> {
        let emu = self.emulator.read().await;
        let buf = emu.snapshot();
        crate::vtty::renderer::VttyRenderer::to_png(&buf, font_size, font_path)
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

    /// Return the current buffer generation counter (O(1) read).
    /// Used by the web UI to skip redundant DOM updates when nothing changed.
    pub async fn buffer_generation(&self) -> u64 {
        let emu = self.emulator.read().await;
        emu.buffer_generation()
    }

    /// Return scrollback line count without cloning the buffer.
    /// Uses the cheap scrollback_len() method on the emulator.
    pub async fn scrollback_count(&self) -> usize {
        let emu = self.emulator.read().await;
        emu.scrollback_len()
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

    /// All VTTY metadata in a single read lock acquisition.
    /// Replaces 8+ separate `emulator.read().await` calls with one.
    pub async fn vtty_metadata(&self) -> VttyMetadata {
        let emu = self.emulator.read().await;
        VttyMetadata {
            cursor: emu.cursor(),
            dimensions: emu.dimensions(),
            scrollback_lines: emu.scrollback_len(),
            alternate_screen: emu.is_alternate_screen(),
            cursor_visible: emu.is_cursor_visible(),
            mouse_tracking: emu.mouse_tracking_enabled(),
            mouse_sgr: emu.mouse_sgr_enabled(),
            generation: emu.buffer_generation(),
        }
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

        // Resize the in-memory VTTY buffer.
        let mut emu = self.emulator.write().await;
        emu.resize(rows as usize, cols as usize);
        drop(emu);

        // Notify sinks so push-mode (WebSocket) clients get a vtty_dirty
        // signal even if the child produces no new output after SIGWINCH.
        let emu = self.emulator.read().await;
        let buf = emu.buffer();
        self.vtty_output.notify_sinks(&buf);

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

    /// Compute a cell-level diff of the current buffer against the last
    /// transmitted snapshot, and return the diff along with cursor and dimensions.
    /// Used by the HTTP diff endpoint and the Level 3 incremental update path.
    pub async fn vtty_diff_and_state(&self) -> (
        crate::vtty::buffer::BufferDiff,
        (usize, usize),
        (usize, usize),
        u64,
    ) {
        let emu = self.emulator.read().await;
        let buf = emu.snapshot();
        let cursor = emu.cursor();
        let dims = emu.dimensions();
        let gen = emu.buffer_generation();
        drop(emu);

        let mut prev_guard = self.prev_diff_snapshot.lock().await;

        match prev_guard.take() {
            None => {
                // First diff — return all cells as changed, store current as baseline
                let diff = crate::vtty::buffer::BufferDiff {
                    width: buf.width,
                    height: buf.height,
                    changed_count: buf.width * buf.height,
                    cells: buf
                        .rows
                        .iter()
                        .enumerate()
                        .flat_map(|(row_idx, row)| {
                            row.iter().enumerate().map(move |(col_idx, cell)| {
                                crate::vtty::buffer::CellDiff {
                                    row: row_idx,
                                    col: col_idx,
                                    ch: cell.ch,
                                    fg: cell.fg,
                                    bg: cell.bg,
                                    bold: cell.bold,
                                    italic: cell.italic,
                                    underline: cell.underline,
                                    blink: cell.blink,
                                    reverse: cell.reverse,
                                    invisible: cell.invisible,
                                    strikethrough: cell.strikethrough,
                                    width: cell.width,
                                }
                            })
                        })
                        .collect(),
                };
                *prev_guard = Some(buf);
                (diff, cursor, dims, gen)
            }
            Some(prev_buf) => {
                let diff = buf.diff(&prev_buf);
                *prev_guard = Some(buf);
                (diff, cursor, dims, gen)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    /// A mock PtyMaster for testing — all operations are no-ops.
    struct MockPtyMaster;

    impl PtyMaster for MockPtyMaster {
        fn clone_reader(&self) -> Result<Box<dyn Read + Send>> {
            Ok(Box::new(std::io::empty()))
        }
        fn take_writer(&self) -> Result<Box<dyn Write + Send>> {
            Ok(Box::new(std::io::sink()))
        }
        fn resize(&self, _rows: u16, _cols: u16) -> Result<()> {
            Ok(())
        }
    }

    /// Build a CommandHandle for testing with a live stdin channel.
    fn make_test_handle(cmd_id: &str) -> (CommandHandle, mpsc::Receiver<StdinMessage>) {
        let (stdin_tx, stdin_rx) = mpsc::channel::<StdinMessage>(16);
        let (_exit_tx, exit_rx) = oneshot::channel::<ExitStatus>();
        let (watch_tx, watch_rx) = tokio::sync::watch::channel(false);
        let emu = VttyEmulator::new(24, 80, 1000);
        let handle = CommandHandle {
            id: cmd_id.to_string(),
            pid: 42,
            name: "test-cmd".to_string(),
            args: vec!["--arg1".to_string()],
            emulator: Arc::new(RwLock::new(emu)),
            stdin_tx,
            _exit_rx: exit_rx,
            handle_registry: HandleRegistry::new(),
            certificate: None,
            exit_config: ExitConfig::default(),
            spawn_time: std::time::Instant::now(),
            pty_master: Arc::new(parking_lot::Mutex::new(Box::new(MockPtyMaster) as Box<dyn PtyMaster + Send>)),
            vtty_output: Arc::new(VttyOutput::new()),
            exit_rx: watch_rx,
            exit_code: std::sync::Mutex::new(None),
            exit_time: std::sync::Mutex::new(None),
            frozen: std::sync::atomic::AtomicBool::new(false),
            prev_diff_snapshot: Mutex::new(None),
        };
        // Keep watch_tx alive so the receiver works
        std::mem::forget(watch_tx);
        (handle, stdin_rx)
    }

    /// Build a CommandHandle for testing with a CLOSED stdin channel.
    fn make_test_handle_no_channel(cmd_id: &str) -> CommandHandle {
        let (stdin_tx, stdin_rx) = mpsc::channel::<StdinMessage>(16);
        drop(stdin_rx); // close the channel immediately
        let (_exit_tx, exit_rx) = oneshot::channel::<ExitStatus>();
        let (watch_tx, watch_rx) = tokio::sync::watch::channel(false);
        let emu = VttyEmulator::new(24, 80, 1000);
        std::mem::forget(watch_tx);
        CommandHandle {
            id: cmd_id.to_string(),
            pid: 42,
            name: "test-cmd".to_string(),
            args: vec![],
            emulator: Arc::new(RwLock::new(emu)),
            stdin_tx,
            _exit_rx: exit_rx,
            handle_registry: HandleRegistry::new(),
            certificate: None,
            exit_config: ExitConfig::default(),
            spawn_time: std::time::Instant::now(),
            pty_master: Arc::new(parking_lot::Mutex::new(Box::new(MockPtyMaster) as Box<dyn PtyMaster + Send>)),
            vtty_output: Arc::new(VttyOutput::new()),
            exit_rx: watch_rx,
            exit_code: std::sync::Mutex::new(None),
            exit_time: std::sync::Mutex::new(None),
            frozen: std::sync::atomic::AtomicBool::new(false),
            prev_diff_snapshot: Mutex::new(None),
        }
    }

    // ─── VttyMetadata struct tests ───

    #[test]
    fn test_vtty_metadata_fields() {
        let meta = VttyMetadata {
            cursor: (10, 20),
            dimensions: (80, 24),
            scrollback_lines: 100,
            alternate_screen: false,
            cursor_visible: true,
            mouse_tracking: false,
            mouse_sgr: false,
            generation: 42,
        };
        assert_eq!(meta.cursor, (10, 20));
        assert_eq!(meta.dimensions, (80, 24));
        assert_eq!(meta.scrollback_lines, 100);
        assert!(!meta.alternate_screen);
        assert!(meta.cursor_visible);
        assert!(!meta.mouse_tracking);
        assert!(!meta.mouse_sgr);
        assert_eq!(meta.generation, 42);
    }

    #[test]
    fn test_vtty_metadata_with_alternate_screen() {
        let meta = VttyMetadata {
            cursor: (0, 0),
            dimensions: (120, 40),
            scrollback_lines: 0,
            alternate_screen: true,
            cursor_visible: true,
            mouse_tracking: true,
            mouse_sgr: true,
            generation: 0,
        };
        assert!(meta.alternate_screen);
        assert!(meta.mouse_tracking);
        assert!(meta.mouse_sgr);
        assert_eq!(meta.dimensions, (120, 40));
    }

    // ─── send_bytes tests ───

    #[tokio::test]
    async fn test_send_bytes_success() {
        let (handle, mut rx) = make_test_handle("test-send-bytes");
        handle.send_bytes(b"hello".to_vec()).await.unwrap();
        let msg = rx.recv().await.unwrap();
        match msg {
            StdinMessage::Bytes(data) => assert_eq!(data, b"hello"),
            _ => panic!("expected Bytes message"),
        }
    }

    #[tokio::test]
    async fn test_send_bytes_channel_closed() {
        let handle = make_test_handle_no_channel("test-send-bytes-closed");
        let result = handle.send_bytes(b"hello".to_vec()).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            ProcessError::ChannelClosed(id) => assert_eq!(id, "test-send-bytes-closed"),
            other => panic!("unexpected error: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_send_bytes_empty() {
        let (handle, mut rx) = make_test_handle("test-send-empty");
        handle.send_bytes(vec![]).await.unwrap();
        let msg = rx.recv().await.unwrap();
        match msg {
            StdinMessage::Bytes(data) => assert!(data.is_empty()),
            _ => panic!("expected Bytes"),
        }
    }

    // ─── send_signal tests ───

    #[tokio::test]
    async fn test_send_signal_success() {
        let (handle, mut rx) = make_test_handle("test-send-signal");
        handle.send_signal("SIGTERM".to_string()).await.unwrap();
        let msg = rx.recv().await.unwrap();
        match msg {
            StdinMessage::Signal(sig) => assert_eq!(sig, "SIGTERM"),
            _ => panic!("expected Signal message"),
        }
    }

    #[tokio::test]
    async fn test_send_signal_channel_closed() {
        let handle = make_test_handle_no_channel("test-signal-closed");
        let result = handle.send_signal("SIGKILL".to_string()).await;
        assert!(result.is_err());
    }

    // ─── kill tests ───

    #[tokio::test]
    async fn test_kill_sends_ctrl_c() {
        let (handle, mut rx) = make_test_handle("test-kill");
        handle.kill().await.unwrap();
        let msg = rx.recv().await.unwrap();
        match msg {
            StdinMessage::Bytes(data) => assert_eq!(data, vec![0x03]),
            _ => panic!("expected Bytes with 0x03"),
        }
    }

    #[tokio::test]
    async fn test_kill_channel_closed() {
        let handle = make_test_handle_no_channel("test-kill-closed");
        let result = handle.kill().await;
        assert!(result.is_err());
    }

    // ─── vtty_snapshot tests ───

    #[tokio::test]
    async fn test_vtty_snapshot_returns_buffer() {
        let (handle, _rx) = make_test_handle("test-snapshot");
        let mut emu = handle.emulator.write().await;
        emu.feed_str("Hello World");
        drop(emu);
        let buf = handle.vtty_snapshot().await;
        assert_eq!(buf.width, 80);
        assert_eq!(buf.height, 24);
        assert_eq!(buf.rows[0][0].ch, 'H');
    }

    #[tokio::test]
    async fn test_vtty_snapshot_empty() {
        let (handle, _rx) = make_test_handle("test-snapshot-empty");
        let buf = handle.vtty_snapshot().await;
        // All cells should be default (space)
        assert_eq!(buf.rows[0][0].ch, ' ');
    }

    // ─── vtty_snapshot_blocking tests ───

    #[tokio::test(flavor = "multi_thread")]
    async fn test_vtty_snapshot_blocking() {
        let (handle, _rx) = make_test_handle("test-snapshot-block");
        {
            let mut emu = handle.emulator.write().await;
            emu.feed_str("Blocking");
        }
        let buf = handle.vtty_snapshot_blocking();
        assert_eq!(buf.rows[0][0].ch, 'B');
        assert_eq!(buf.rows[0][7].ch, 'g');
    }

    // ─── vtty_plain tests ───

    #[tokio::test]
    async fn test_vtty_plain() {
        let (handle, _rx) = make_test_handle("test-plain");
        {
            let mut emu = handle.emulator.write().await;
            emu.feed_str("AB\nCD");
        }
        let plain = handle.vtty_plain().await;
        assert!(plain.contains("AB"));
        assert!(plain.contains("CD"));
    }

    #[tokio::test]
    async fn test_vtty_plain_empty() {
        let (handle, _rx) = make_test_handle("test-plain-empty");
        let plain = handle.vtty_plain().await;
        // Empty buffer should not panic, just return whitespace/newlines
        assert!(plain.is_empty() || plain.chars().all(|c| c == ' ' || c == '\n'));
    }

    // ─── vtty_ansi tests ───

    #[tokio::test]
    async fn test_vtty_ansi() {
        let (handle, _rx) = make_test_handle("test-ansi");
        {
            let mut emu = handle.emulator.write().await;
            emu.feed_str("X");
        }
        let ansi = handle.vtty_ansi().await;
        assert!(!ansi.is_empty());
        // Should contain ANSI reset codes
        assert!(ansi.contains("\x1b[") || ansi.contains("X"));
    }

    // ─── vtty_partial tests ───

    #[tokio::test]
    async fn test_vtty_partial() {
        let (handle, _rx) = make_test_handle("test-partial");
        {
            let mut emu = handle.emulator.write().await;
            emu.feed_str("Row0\nRow1\nRow2");
        }
        let partial = handle.vtty_partial(1, 1).await;
        assert!(partial.contains("Row1") || partial.contains("Row1".trim_end()));
    }

    #[tokio::test]
    async fn test_vtty_partial_out_of_bounds() {
        let (handle, _rx) = make_test_handle("test-partial-oob");
        // Requesting beyond buffer size should not panic
        let partial = handle.vtty_partial(100, 5).await;
        assert!(partial.is_empty());
    }

    // ─── vtty_html tests ───

    #[tokio::test]
    async fn test_vtty_html() {
        let (handle, _rx) = make_test_handle("test-html");
        {
            let mut emu = handle.emulator.write().await;
            emu.feed_str("HI");
        }
        let html = handle.vtty_html().await;
        assert!(html.contains("<"));
    }

    // ─── vtty_html_scrollback tests ───

    #[tokio::test]
    async fn test_vtty_html_scrollback() {
        let (handle, _rx) = make_test_handle("test-scrollback-html");
        {
            let mut emu = handle.emulator.write().await;
            emu.feed_str("SCROLL");
        }
        let html = handle.vtty_html_scrollback(0, 5).await;
        assert!(html.contains("<"));
    }

    // ─── is_alternate_screen tests ───

    #[tokio::test]
    async fn test_is_alternate_screen_false() {
        let (handle, _rx) = make_test_handle("test-alt-screen");
        assert!(!handle.is_alternate_screen().await);
    }

    #[tokio::test]
    async fn test_is_alternate_screen_true() {
        let (handle, _rx) = make_test_handle("test-alt-screen-on");
        {
            let mut emu = handle.emulator.write().await;
            emu.feed(b"\x1b[?1049h"); // enter alt screen
        }
        assert!(handle.is_alternate_screen().await);
    }

    // ─── vtty_html_main tests ───

    #[tokio::test]
    async fn test_vtty_html_main() {
        let (handle, _rx) = make_test_handle("test-html-main");
        let html = handle.vtty_html_main().await;
        assert!(html.contains("<"));
    }

    // ─── vtty_html_alt tests ───

    #[tokio::test]
    async fn test_vtty_html_alt_empty() {
        let (handle, _rx) = make_test_handle("test-html-alt");
        let html = handle.vtty_html_alt().await;
        // No alt screen used → should produce valid (possibly empty) HTML
        assert!(html.contains("<"));
    }

    // ─── cursor_position tests ───

    #[tokio::test]
    async fn test_cursor_position_initial() {
        let (handle, _rx) = make_test_handle("test-cursor-pos");
        let pos = handle.cursor_position().await;
        assert_eq!(pos, (0, 0));
    }

    #[tokio::test]
    async fn test_cursor_position_after_text() {
        let (handle, _rx) = make_test_handle("test-cursor-pos2");
        {
            let mut emu = handle.emulator.write().await;
            emu.feed_str("ABCDE");
        }
        let pos = handle.cursor_position().await;
        assert_eq!(pos, (0, 5));
    }

    // ─── is_cursor_visible tests ───

    #[tokio::test]
    async fn test_is_cursor_visible_default() {
        let (handle, _rx) = make_test_handle("test-cursor-vis");
        assert!(handle.is_cursor_visible().await);
    }

    #[tokio::test]
    async fn test_is_cursor_visible_hidden() {
        let (handle, _rx) = make_test_handle("test-cursor-hidden");
        {
            let mut emu = handle.emulator.write().await;
            emu.feed(b"\x1b[?25l"); // hide cursor
        }
        assert!(!handle.is_cursor_visible().await);
    }

    // ─── cursor_style tests ───

    #[tokio::test]
    async fn test_cursor_style_default() {
        let (handle, _rx) = make_test_handle("test-cursor-style");
        let style = handle.cursor_style().await;
        assert_eq!(style, crate::vtty::emulator::CursorStyle::Block(true));
    }

    #[tokio::test]
    async fn test_cursor_style_underline() {
        let (handle, _rx) = make_test_handle("test-cursor-style2");
        {
            let mut emu = handle.emulator.write().await;
            emu.feed(b"\x1b[3 q"); // underline cursor
        }
        let style = handle.cursor_style().await;
        assert_eq!(style, crate::vtty::emulator::CursorStyle::Underline(true));
    }

    // ─── send_paste tests ───

    #[tokio::test]
    async fn test_send_paste_no_bracketed() {
        let (handle, mut rx) = make_test_handle("test-paste-no-bracket");
        handle.send_paste("pasted text").await.unwrap();
        let msg = rx.recv().await.unwrap();
        match msg {
            StdinMessage::Bytes(data) => {
                assert_eq!(String::from_utf8_lossy(&data), "pasted text");
            }
            _ => panic!("expected Bytes"),
        }
    }

    #[tokio::test]
    async fn test_send_paste_with_bracketed() {
        let (handle, mut rx) = make_test_handle("test-paste-bracket");
        {
            let mut emu = handle.emulator.write().await;
            emu.feed(b"\x1b[?2004h"); // enable bracketed paste
        }
        handle.send_paste("pasted").await.unwrap();
        let msg = rx.recv().await.unwrap();
        match msg {
            StdinMessage::Bytes(data) => {
                let s = String::from_utf8_lossy(&data);
                assert!(s.starts_with("\x1b[200~"));
                assert!(s.ends_with("\x1b[201~"));
                assert!(s.contains("pasted"));
            }
            _ => panic!("expected Bytes"),
        }
    }

    // ─── dimensions tests ───

    #[tokio::test]
    async fn test_dimensions() {
        let (handle, _rx) = make_test_handle("test-dims");
        let dims = handle.dimensions().await;
        assert_eq!(dims, (24, 80)); // (rows, cols)
    }

    // ─── buffer_generation tests ───

    #[tokio::test]
    async fn test_buffer_generation_initial() {
        let (handle, _rx) = make_test_handle("test-gen-init");
        let gen = handle.buffer_generation().await;
        assert_eq!(gen, 0);
    }

    #[tokio::test]
    async fn test_buffer_generation_increments() {
        let (handle, _rx) = make_test_handle("test-gen-incr");
        let gen0 = handle.buffer_generation().await;
        {
            let mut emu = handle.emulator.write().await;
            emu.feed_str("X");
        }
        let gen1 = handle.buffer_generation().await;
        assert!(gen1 > gen0);
    }

    // ─── scrollback_count tests ───

    #[tokio::test]
    async fn test_scrollback_count_initial() {
        let (handle, _rx) = make_test_handle("test-scroll-init");
        assert_eq!(handle.scrollback_count().await, 0);
    }

    #[tokio::test]
    async fn test_scrollback_count_after_scroll() {
        let (handle, _rx) = make_test_handle("test-scroll-incr");
        {
            // Fill the screen and scroll
            let mut emu = handle.emulator.write().await;
            for i in 0..30 {
                emu.feed_str(&format!("Line {}\n", i));
            }
        }
        assert!(handle.scrollback_count().await > 0);
    }

    // ─── mouse_tracking_enabled tests ───

    #[tokio::test]
    async fn test_mouse_tracking_default_false() {
        let (handle, _rx) = make_test_handle("test-mouse-default");
        assert!(!handle.mouse_tracking_enabled().await);
    }

    #[tokio::test]
    async fn test_mouse_tracking_enabled() {
        let (handle, _rx) = make_test_handle("test-mouse-on");
        {
            let mut emu = handle.emulator.write().await;
            emu.feed(b"\x1b[?1002h"); // enable button tracking
        }
        assert!(handle.mouse_tracking_enabled().await);
    }

    // ─── mouse_sgr_enabled tests ───

    #[tokio::test]
    async fn test_mouse_sgr_default_false() {
        let (handle, _rx) = make_test_handle("test-sgr-default");
        assert!(!handle.mouse_sgr_enabled().await);
    }

    #[tokio::test]
    async fn test_mouse_sgr_enabled() {
        let (handle, _rx) = make_test_handle("test-sgr-on");
        {
            let mut emu = handle.emulator.write().await;
            emu.feed(b"\x1b[?1006h");
        }
        assert!(handle.mouse_sgr_enabled().await);
    }

    // ─── vtty_metadata tests ───

    #[tokio::test]
    async fn test_vtty_metadata_from_handle() {
        let (handle, _rx) = make_test_handle("test-meta-handle");
        {
            let mut emu = handle.emulator.write().await;
            emu.feed_str("Meta");
        }
        let meta = handle.vtty_metadata().await;
        assert_eq!(meta.cursor, (0, 4)); // after writing "Meta"
        assert_eq!(meta.dimensions, (24, 80));
        assert_eq!(meta.scrollback_lines, 0);
        assert!(!meta.alternate_screen);
        assert!(meta.cursor_visible);
        assert!(!meta.mouse_tracking);
        assert!(!meta.mouse_sgr);
        assert!(meta.generation > 0);
    }

    // ─── resize tests ───

    #[tokio::test]
    async fn test_resize() {
        let (handle, _rx) = make_test_handle("test-resize");
        handle.resize(40, 120).await.unwrap();
        let dims = handle.dimensions().await;
        assert_eq!(dims, (40, 120));
    }

    // ─── resize_pty tests ───

    #[tokio::test]
    async fn test_resize_pty() {
        let (handle, _rx) = make_test_handle("test-resize-pty");
        handle.resize_pty(50, 132).await.unwrap();
        let dims = handle.dimensions().await;
        assert_eq!(dims, (50, 132));
    }

    // ─── list_handles tests ───

    #[test]
    fn test_list_handles_empty() {
        let (handle, _rx) = make_test_handle("test-list-empty");
        assert!(handle.list_handles().is_empty());
    }

    // ─── runtime_secs tests ───

    #[test]
    fn test_runtime_secs_non_negative() {
        let (handle, _rx) = make_test_handle("test-runtime");
        let rt = handle.runtime_secs();
        assert!(rt >= 0.0);
    }

    #[test]
    fn test_runtime_secs_increases() {
        let (handle, _rx) = make_test_handle("test-runtime-incr");
        let rt1 = handle.runtime_secs();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let rt2 = handle.runtime_secs();
        assert!(rt2 >= rt1);
    }

    // ─── is_alive tests ───

    #[test]
    fn test_is_alive() {
        let (handle, _rx) = make_test_handle("test-alive");
        // PID 42 may or may not exist — just ensure it doesn't panic
        let _ = handle.is_alive();
    }

    // ─── is_frozen tests ───

    #[test]
    fn test_is_frozen_default() {
        let (handle, _rx) = make_test_handle("test-frozen-default");
        assert!(!handle.is_frozen());
    }

    #[test]
    fn test_is_frozen_set() {
        let (handle, _rx) = make_test_handle("test-frozen-set");
        handle.frozen.store(true, std::sync::atomic::Ordering::Relaxed);
        assert!(handle.is_frozen());
    }

    // ─── focus_reporting_enabled tests ───

    #[tokio::test]
    async fn test_focus_reporting_default() {
        let (handle, _rx) = make_test_handle("test-focus-default");
        assert!(!handle.focus_reporting_enabled().await);
    }

    #[tokio::test]
    async fn test_focus_reporting_enabled() {
        let (handle, _rx) = make_test_handle("test-focus-on");
        {
            let mut emu = handle.emulator.write().await;
            emu.feed(b"\x1b[?1004h");
        }
        assert!(handle.focus_reporting_enabled().await);
    }

    // ─── recover_alternate_screen tests ───

    #[tokio::test]
    async fn test_recover_alternate_screen_noop() {
        let (handle, _rx) = make_test_handle("test-recover-noop");
        // Not on alt screen → should be a no-op
        handle.recover_alternate_screen().await;
        assert!(!handle.is_alternate_screen().await);
    }

    #[tokio::test]
    async fn test_recover_alternate_screen_recovers() {
        let (handle, _rx) = make_test_handle("test-recover-yes");
        {
            let mut emu = handle.emulator.write().await;
            emu.feed(b"\x1b[?1049h"); // enter alt screen
        }
        assert!(handle.is_alternate_screen().await);
        handle.recover_alternate_screen().await;
        assert!(!handle.is_alternate_screen().await);
    }

    // ─── vtty_diff_and_state tests ───

    #[tokio::test]
    async fn test_vtty_diff_first_call() {
        let (handle, _rx) = make_test_handle("test-diff-first");
        {
            let mut emu = handle.emulator.write().await;
            emu.feed_str("X");
        }
        let (diff, cursor, dims, gen) = handle.vtty_diff_and_state().await;
        // First diff should return all cells as changed
        assert_eq!(diff.width, 80);
        assert_eq!(diff.height, 24);
        assert_eq!(diff.changed_count, 80 * 24);
        assert_eq!(cursor, (0, 1));
        assert_eq!(dims, (24, 80));
        assert!(gen > 0);
    }

    #[tokio::test]
    async fn test_vtty_diff_second_call_incremental() {
        let (handle, _rx) = make_test_handle("test-diff-second");
        {
            let mut emu = handle.emulator.write().await;
            emu.feed_str("A");
        }
        let (diff1, _, _, _) = handle.vtty_diff_and_state().await;
        assert_eq!(diff1.changed_count, 80 * 24);

        // Second call — only the one changed cell should differ
        let (diff2, _, _, _) = handle.vtty_diff_and_state().await;
        // Both buffers are the same now (no change between calls), so diff should be empty
        assert_eq!(diff2.changed_count, 0);
    }

    #[tokio::test]
    async fn test_vtty_diff_after_change() {
        let (handle, _rx) = make_test_handle("test-diff-change");
        {
            let mut emu = handle.emulator.write().await;
            emu.feed_str("AB");
        }
        let _ = handle.vtty_diff_and_state().await; // baseline

        {
            let mut emu = handle.emulator.write().await;
            emu.feed_str("C");
        }
        let (diff, _, _, _) = handle.vtty_diff_and_state().await;
        // At least 1 cell changed (the 'C' at position (0,2))
        assert!(diff.changed_count >= 1);
    }

    // ─── exit_code / exit_time tests ───

    #[test]
    fn test_exit_code_initial_none() {
        let (handle, _rx) = make_test_handle("test-exit-code-init");
        assert_eq!(*handle.exit_code.lock().unwrap(), None);
    }

    #[test]
    fn test_exit_code_set() {
        let (handle, _rx) = make_test_handle("test-exit-code-set");
        *handle.exit_code.lock().unwrap() = Some(0);
        assert_eq!(*handle.exit_code.lock().unwrap(), Some(0));
    }

    #[test]
    fn test_exit_time_initial_none() {
        let (handle, _rx) = make_test_handle("test-exit-time-init");
        assert_eq!(*handle.exit_time.lock().unwrap(), None);
    }

    #[test]
    fn test_certificate_initial_none() {
        let (handle, _rx) = make_test_handle("test-cert-init");
        assert!(handle.certificate.is_none());
    }

    #[test]
    fn test_certificate_set() {
        let (mut handle, _rx) = make_test_handle("test-cert-set");
        handle.certificate = Some("my-cert".to_string());
        assert_eq!(handle.certificate.as_deref(), Some("my-cert"));
    }

    // ─── handle_registry tests ───

    #[test]
    fn test_handle_registry_integration() {
        let (mut handle, _rx) = make_test_handle("test-registry-integ");
        handle.handle_registry.add("stdout".to_string(), Box::new(crate::handles::null_sink::NullSink));
        let names = handle.list_handles();
        assert_eq!(names.len(), 1);
        assert!(names.contains(&"stdout".to_string()));
    }
}
