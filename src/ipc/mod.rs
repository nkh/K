//! Inter-Process Communication via Unix Domain Sockets.
//!
//! Replaces the HTTP/WebSocket server for CLI-to-instance communication.
//! A running vrc instance listens on a UDS control socket at
//! `~/.local/share/vrc/control-{pid}.sock`.  Other `vrc` CLI
//! invocations connect to this socket to send commands (keys, spawn,
//! kill, freeze, thaw, cat, resize, shutdown, etc.).

pub mod client;
pub mod protocol;
pub mod server;

use std::path::PathBuf;

/// Return the canonical socket path for a given PID.
pub fn socket_path_for_pid(pid: u32) -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("vrc")
        .join(format!("control-{}.sock", pid))
}


