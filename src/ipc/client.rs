//! UDS control socket client.
//!
//! Connects to a running vrc instance's control socket, sends a
//! [`ControlCommand`], and returns the [`ControlResponse`].

use std::path::Path;

use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::UnixStream;

use super::protocol::{decode_frame, encode_frame, ControlCommand, ControlResponse};
use crate::ipc::socket_path_for_pid;

/// Connect to a running vrc instance's control socket and send a command.
///
/// Returns the response, or an error if the connection fails.
pub async fn send_command(pid: u32, cmd: ControlCommand) -> anyhow::Result<ControlResponse> {
    let socket_path = socket_path_for_pid(pid);
    send_command_to_path(&socket_path, cmd).await
}

/// Connect to a specific socket path and send a command.
pub async fn send_command_to_path(
    socket_path: &Path,
    cmd: ControlCommand,
) -> anyhow::Result<ControlResponse> {
    // Verify socket exists
    if !socket_path.exists() {
        anyhow::bail!(
            "No control socket at {} — is vrc instance running?",
            socket_path.display()
        );
    }

    let mut stream = UnixStream::connect(socket_path).await.map_err(|e| {
        anyhow::anyhow!(
            "Failed to connect to control socket at {}: {}",
            socket_path.display(),
            e
        )
    })?;

    // Send the command frame
    let frame = encode_frame(&cmd)?;
    stream.write_all(&frame).await?;
    stream.flush().await?;

    // Read the response frame
    let mut read_buf = Vec::new();
    let mut tmp = [0u8; 65536];
    loop {
        match stream.read(&mut tmp).await {
            Ok(0) => break,
            Ok(n) => read_buf.extend_from_slice(&tmp[..n]),
            Err(e) => {
                anyhow::bail!("Read error on control socket: {}", e);
            }
        }

        // Try to decode a frame
        if let Some((_frame_end, payload)) = decode_frame(&read_buf) {
            let response: ControlResponse = serde_json::from_slice(&payload)?;
            return Ok(response);
        }
    }

    anyhow::bail!("Connection closed before receiving response")
}

/// Send a Ping command to verify an instance is alive.
/// Returns instance info on success.
pub async fn ping(pid: u32) -> anyhow::Result<serde_json::Value> {
    let response = send_command(pid, ControlCommand::Ping).await?;
    match response {
        ControlResponse::Ok { data } => Ok(data),
        ControlResponse::Error { error } => anyhow::bail!("{}", error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc::protocol::{ControlCommand, ControlResponse};

    #[tokio::test]
    async fn test_send_command_to_nonexistent_socket() {
        let result = send_command(999999999, ControlCommand::Ping).await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("No control socket") || err_msg.contains("Failed to connect"));
    }

    #[tokio::test]
    async fn test_send_command_to_invalid_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nonexistent.sock");
        let result = send_command_to_path(&path, ControlCommand::Ping).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_ping_nonexistent_instance() {
        let result = ping(999999999).await;
        assert!(result.is_err());
    }
}
