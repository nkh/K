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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_socket_path_for_pid() {
        let path = socket_path_for_pid(12345);
        assert!(path.to_string_lossy().contains("12345"));
        assert!(path.to_string_lossy().contains("control-"));
        assert!(path.to_string_lossy().contains(".sock"));
    }

    #[test]
    fn test_socket_path_contains_data_dir() {
        let path = socket_path_for_pid(1);
        let path_str = path.to_string_lossy().to_string();
        assert!(path_str.contains("vrc"));
    }

    #[test]
    fn test_socket_path_is_pathbuf() {
        let path = socket_path_for_pid(42);
        assert!(path.is_absolute() || path.components().count() >= 3);
    }
}
