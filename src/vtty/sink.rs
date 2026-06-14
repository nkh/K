//! VTTY output sink abstraction.
//!
//! This module provides the [`VttySink`] trait, which abstracts how VTTY
//! buffer changes are delivered to consumers.  Instead of every consumer
//! polling the emulator on a timer, sinks receive push notifications when
//! the buffer is updated.
//!
//! # Architecture
//!
//! ```text
//!   PTY reader → emulator.feed() → buffer snapshot → VttyOutput.notify_sinks()
//!                                                         ├─ BroadcastVttySink  → tokio broadcast channel
//!                                                         ├─ InMemoryVttySink   → Arc<RwLock<Option<Buffer>>>
//!                                                         └─ LogVttySink        → append to file
//! ```
//!
//! The [`VttyOutput`] struct owns a list of sinks and is responsible for
//! calling [`VttySink::on_buffer_change`] after each emulator feed.  It is
//! stored in [`CommandHandle`](crate::process::handle::CommandHandle) and
//! wired into the PTY consumer task in the spawner.

use super::buffer::Buffer;
use std::io::Write as _;
use std::sync::Arc;
use tokio::sync::broadcast;

// ---------------------------------------------------------------------------
// Trait
// ---------------------------------------------------------------------------

/// A receiver for VTTY buffer-change notifications.
///
/// Implementors are notified synchronously from the PTY output consumer
/// after each call to [`VttyEmulator::feed`](crate::vtty::emulator::VttyEmulator::feed).
/// Implementations **must** remain non-blocking — prefer fire-and-forget
/// channel sends over synchronous I/O.
///
/// # Lifecycle
///
/// 1. `on_buffer_change` — called once per feed with a buffer snapshot.
/// 2. `on_close` — called when the PTY closes / the VTTY is torn down.
pub trait VttySink: Send + Sync {
    /// Called when the VTTY buffer has changed.
    ///
    /// `buffer` is a snapshot of the current buffer state.  The receiver
    /// may clone it or extract only the information it needs.
    fn on_buffer_change(&self, buffer: &Buffer);

    /// Called when the VTTY is being shut down (PTY closed / EOF).
    ///
    /// The default implementation does nothing.
    fn on_close(&self) {}
}

// ---------------------------------------------------------------------------
// VttyOutput — owns and drives a list of sinks
// ---------------------------------------------------------------------------

/// Manages a collection of [`VttySink`] instances attached to a single VTTY.
///
/// After feeding new data to the emulator, call [`VttyOutput::notify_sinks`]
/// to push the updated buffer to every registered sink.
///
/// `VttyOutput` is cheaply clonable — the sink list is shared via `Arc`.
#[derive(Clone, Default)]
pub struct VttyOutput {
    sinks: Arc<Vec<Arc<dyn VttySink>>>,
}

impl VttyOutput {
    /// Create a new `VttyOutput` with no sinks.
    pub fn new() -> Self {
        Self {
            sinks: Arc::new(Vec::new()),
        }
    }

    /// Create a `VttyOutput` pre-loaded with the given sinks.
    pub fn with_sinks(sinks: Vec<Arc<dyn VttySink>>) -> Self {
        Self {
            sinks: Arc::new(sinks),
        }
    }

    /// Add a sink to the output.
    ///
    /// This replaces the internal `Arc<Vec>` (copy-on-write semantics), so
    /// it is not suitable for high-frequency calls.  Prefer
    /// [`VttyOutput::with_sinks`] to build the list once at construction.
    pub fn add_sink(&mut self, sink: Arc<dyn VttySink>) {
        let mut list = Arc::try_unwrap(std::mem::replace(&mut self.sinks, Arc::new(Vec::new())))
            .unwrap_or_else(|arc| (*arc).clone());
        list.push(sink);
        self.sinks = Arc::new(list);
    }

    /// Notify all registered sinks that the buffer has changed.
    pub fn notify_sinks(&self, buffer: &Buffer) {
        for sink in self.sinks.iter() {
            sink.on_buffer_change(buffer);
        }
    }

    /// Notify all sinks that the VTTY is closing.
    pub fn close(&self) {
        for sink in self.sinks.iter() {
            sink.on_close();
        }
    }

    /// Return the number of registered sinks.
    pub fn sink_count(&self) -> usize {
        self.sinks.len()
    }
}

// ---------------------------------------------------------------------------
// BroadcastVttySink — push notifications via tokio broadcast
// ---------------------------------------------------------------------------

/// A [`VttySink`] that broadcasts lightweight "dirty" signals via a
/// [`tokio::sync::broadcast`] channel.
///
/// Each notification is a `(command_id, json_string)` pair.  The JSON
/// **does not** include cell data — consumers are expected to fetch the
/// latest content via a separate HTTP request (e.g.
/// `GET /api/commands/:id/vtty/html`).
///
/// This replaces the polling-based diff watcher with a push model:
/// the sink fires immediately when the buffer changes, eliminating the
/// polling interval latency.
pub struct BroadcastVttySink {
    tx: broadcast::Sender<(String, String)>,
    command_id: String,
}

impl BroadcastVttySink {
    /// Create a new broadcast sink.
    ///
    /// * `tx` — broadcast sender shared with consumers (WebSocket handler,
    ///   log stream, etc.)
    /// * `command_id` — included in every notification so consumers can
    ///   correlate events to specific commands.
    pub fn new(tx: broadcast::Sender<(String, String)>, command_id: String) -> Self {
        Self { tx, command_id }
    }

    /// Obtain a new receiver for this broadcast channel.
    pub fn subscribe(&self) -> broadcast::Receiver<(String, String)> {
        self.tx.subscribe()
    }
}

impl VttySink for BroadcastVttySink {
    fn on_buffer_change(&self, _buffer: &Buffer) {
        let msg = serde_json::json!({
            "type": "vtty_dirty",
            "data": { "id": &self.command_id }
        })
        .to_string();
        // Best-effort, non-blocking send.  If all receivers are lagged
        // or dropped the notification is silently discarded.
        let _ = self.tx.send((self.command_id.clone(), msg));
    }

    fn on_close(&self) {
        let msg = serde_json::json!({
            "type": "vtty_close",
            "data": { "id": &self.command_id }
        })
        .to_string();
        let _ = self.tx.send((self.command_id.clone(), msg));
    }
}

// ---------------------------------------------------------------------------
// InMemoryVttySink — stores latest snapshot (testing / introspection)
// ---------------------------------------------------------------------------

/// A [`VttySink`] that stores the most recent buffer snapshot in memory.
///
/// Useful for:
/// - **Testing**: assert on emulator output without polling.
/// - **Introspection**: inspect the last known state synchronously.
/// - **Debugging**: capture terminal output for later analysis.
pub struct InMemoryVttySink {
    latest: parking_lot::RwLock<Option<Buffer>>,
    change_count: parking_lot::Mutex<usize>,
}

impl InMemoryVttySink {
    /// Create a new in-memory sink.
    pub fn new() -> Self {
        Self {
            latest: parking_lot::RwLock::new(None),
            change_count: parking_lot::Mutex::new(0),
        }
    }

    /// Get a clone of the latest buffer snapshot, if any update has been received.
    pub fn latest(&self) -> Option<Buffer> {
        self.latest.read().clone()
    }

    /// How many times [`VttySink::on_buffer_change`] has been called.
    pub fn change_count(&self) -> usize {
        *self.change_count.lock()
    }

    /// Reset the sink: clear the stored snapshot and reset the counter.
    pub fn reset(&self) {
        *self.latest.write() = None;
        *self.change_count.lock() = 0;
    }
}

impl Default for InMemoryVttySink {
    fn default() -> Self {
        Self::new()
    }
}

impl VttySink for InMemoryVttySink {
    fn on_buffer_change(&self, buffer: &Buffer) {
        *self.latest.write() = Some(buffer.clone());
        *self.change_count.lock() += 1;
    }

    fn on_close(&self) {
        *self.latest.write() = None;
    }
}

// ---------------------------------------------------------------------------
// LogVttySink — append rendered output to a file
// ---------------------------------------------------------------------------

/// A [`VttySink`] that appends plain-text VTTY output to a log file.
///
/// Each buffer change is rendered as plain text and appended to the file
/// with a separator line.  Useful for audit trails or post-mortem analysis.
pub struct LogVttySink {
    file: parking_lot::Mutex<std::fs::File>,
}

impl LogVttySink {
    /// Create a new log sink writing to `path`.
    ///
    /// The file is created if it doesn't exist; new content is appended.
    pub fn new(path: &str) -> std::io::Result<Self> {
        use std::fs::OpenOptions;
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        Ok(Self {
            file: parking_lot::Mutex::new(file),
        })
    }
}

impl VttySink for LogVttySink {
    fn on_buffer_change(&self, buffer: &Buffer) {
        let mut file = self.file.lock();
        for row in &buffer.rows {
            let line: String = row.iter().map(|c| c.ch).collect();
            let _ = writeln!(file, "{}", line);
        }
        let _ = writeln!(file, "---");
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vtty::emulator::VttyEmulator;

    // -- VttyOutput tests --

    #[test]
    fn test_vtty_output_multiple_sinks() {
        let sink1 = Arc::new(InMemoryVttySink::new());
        let sink2 = Arc::new(InMemoryVttySink::new());
        let output = VttyOutput::with_sinks(vec![sink1.clone(), sink2.clone()]);

        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("Test");
        output.notify_sinks(&emu.snapshot());

        assert_eq!(sink1.change_count(), 1);
        assert_eq!(sink2.change_count(), 1);

        // Both sinks should have captured the same buffer
        assert_eq!(sink1.latest().unwrap().rows[0][0].ch, 'T');
        assert_eq!(sink2.latest().unwrap().rows[0][0].ch, 'T');
    }

    #[test]
    fn test_vtty_output_empty_no_panic() {
        let output = VttyOutput::new();
        let mut emu = VttyEmulator::new(5, 10, 100);
        emu.feed_str("Hello");
        // Must not panic with zero sinks
        output.notify_sinks(&emu.snapshot());
        output.close();
    }

    // -- Integration: VttyOutput + emulator lifecycle --

    #[test]
    fn test_full_lifecycle() {
        // Simulates the real PTY consumer lifecycle:
        //   feed → notify → feed → notify → close
        let sink = Arc::new(InMemoryVttySink::new());
        let output = VttyOutput::with_sinks(vec![sink.clone()]);
        let mut emu = VttyEmulator::new(5, 20, 100);

        // First chunk from PTY
        emu.feed(b"echo hello\r\n");
        output.notify_sinks(&emu.snapshot());
        assert_eq!(sink.change_count(), 1);

        // Second chunk
        emu.feed(b"$ ");
        output.notify_sinks(&emu.snapshot());
        assert_eq!(sink.change_count(), 2);
        let latest = sink.latest().unwrap();
        assert!(latest.rows.iter().any(|row| {
            let text: String = row.iter().map(|c| c.ch).collect();
            text.contains("echo hello")
        }));

        // PTY closes
        output.close();
        assert!(sink.latest().is_none());
    }
}
