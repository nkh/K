//! Wire protocol for UDS IPC.
//!
//! Messages are length-prefixed JSON frames:
//!   [4 bytes big-endian length (u32)] [JSON payload]
//!
//! Each message is either a `ControlCommand` (client → server) or a
//! `ControlResponse` (server → client).

use serde::{Deserialize, Serialize};

/// Commands that a client can send to a running vrunner instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ControlCommand {
    /// List all managed commands.
    List,

    /// Spawn a new command inside the target instance.
    Spawn {
        cmd: String,
        args: Vec<String>,
        env: Option<std::collections::HashMap<String, String>>,
        rows: Option<u16>,
        cols: Option<u16>,
        dir: Option<String>,
    },

    /// Send keystrokes to a managed command.
    SendKeys {
        id: String,
        keys: String,
    },

    /// Kill (terminate) a managed command.
    Kill {
        id: String,
    },

    /// Freeze (SIGSTOP) a managed command.
    Freeze {
        id: String,
    },

    /// Thaw (SIGCONT) a frozen command.
    Thaw {
        id: String,
    },

    /// Purge (remove) a retained/exited command.
    Purge {
        id: String,
    },

    /// Restart a command (spawn same cmd+args, purge old).
    Restart {
        id: String,
    },

    /// Resize the VTTY of a managed command.
    Resize {
        id: String,
        rows: u16,
        cols: u16,
    },

    /// Get VTTY text output (plain text) for a command.
    Cat {
        id: String,
    },

    /// Store a named snapshot of a command's VTTY buffer.
    Snapshot {
        id: String,
        name: String,
    },

    /// List snapshots for a command.
    ListSnapshots {
        id: String,
    },

    /// Delete a named snapshot.
    DeleteSnapshot {
        id: String,
        name: String,
    },

    /// Gracefully shut down the entire vrunner instance.
    Shutdown,

    /// Ping — returns instance info for liveness check.
    Ping,
}

/// Responses sent back from the running instance to the client.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status")]
pub enum ControlResponse {
    Ok { data: serde_json::Value },
    Error { error: String },
}

// ── Framing helpers ──

/// Encode a message as a length-prefixed JSON frame.
pub fn encode_frame(msg: &impl Serialize) -> anyhow::Result<Vec<u8>> {
    let json = serde_json::to_vec(msg)?;
    let len = json.len() as u32;
    let mut frame = Vec::with_capacity(4 + json.len());
    frame.extend_from_slice(&len.to_be_bytes());
    frame.extend_from_slice(&json);
    Ok(frame)
}

/// Decode a single length-prefixed JSON frame from a reader.
/// Returns None if the frame is incomplete (need more data).
/// Returns Err on corruption.
pub fn decode_frame(buf: &[u8]) -> Option<(usize, Vec<u8>)> {
    if buf.len() < 4 {
        return None;
    }
    let len = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
    let frame_end = 4 + len;
    if buf.len() < frame_end {
        return None; // incomplete
    }
    Some((frame_end, buf[4..frame_end].to_vec()))
}
