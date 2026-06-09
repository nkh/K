//! Wire protocol for UDS IPC.
//!
//! Messages are length-prefixed JSON frames:
//!   [4 bytes big-endian length (u32)] [JSON payload]
//!
//! Each message is either a `ControlCommand` (client → server) or a
//! `ControlResponse` (server → client).

use serde::{Deserialize, Serialize};

/// Commands that a client can send to a running vrc instance.
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

    /// Gracefully shut down the entire vrc instance.
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

#[cfg(test)]
mod tests {
    use super::*;

    // ── encode_frame tests ──

    #[test]
    fn encode_command_list() {
        let cmd = ControlCommand::List;
        let frame = encode_frame(&cmd).unwrap();
        // Frame should be 4-byte length prefix + JSON
        assert!(frame.len() > 4);
        let json = &frame[4..];
        let parsed: ControlCommand = serde_json::from_slice(json).unwrap();
        match parsed {
            ControlCommand::List => {}
            _ => panic!("Expected List variant"),
        }
    }

    #[test]
    fn encode_command_spawn() {
        let cmd = ControlCommand::Spawn {
            cmd: "htop".to_string(),
            args: vec!["--sort-key".to_string(), "PERCENT".to_string()],
            env: None,
            rows: Some(50),
            cols: Some(200),
            dir: Some("/tmp".to_string()),
        };
        let frame = encode_frame(&cmd).unwrap();
        let json = &frame[4..];
        let parsed: ControlCommand = serde_json::from_slice(json).unwrap();
        match parsed {
            ControlCommand::Spawn { cmd, args, env, rows, cols, dir } => {
                assert_eq!(cmd, "htop");
                assert_eq!(args, vec!["--sort-key", "PERCENT"]);
                assert!(env.is_none());
                assert_eq!(rows, Some(50));
                assert_eq!(cols, Some(200));
                assert_eq!(dir, Some("/tmp".to_string()));
            }
            _ => panic!("Expected Spawn variant"),
        }
    }

    #[test]
    fn encode_command_send_keys() {
        let cmd = ControlCommand::SendKeys {
            id: "abc123".to_string(),
            keys: "ctrl-c".to_string(),
        };
        let frame = encode_frame(&cmd).unwrap();
        let json = &frame[4..];
        let parsed: ControlCommand = serde_json::from_slice(json).unwrap();
        match parsed {
            ControlCommand::SendKeys { id, keys } => {
                assert_eq!(id, "abc123");
                assert_eq!(keys, "ctrl-c");
            }
            _ => panic!("Expected SendKeys variant"),
        }
    }

    #[test]
    fn encode_response_ok() {
        let resp = ControlResponse::Ok {
            data: serde_json::json!({"pid": 12345}),
        };
        let frame = encode_frame(&resp).unwrap();
        let json = &frame[4..];
        let parsed: ControlResponse = serde_json::from_slice(json).unwrap();
        match parsed {
            ControlResponse::Ok { data } => {
                assert_eq!(data["pid"], 12345);
            }
            _ => panic!("Expected Ok variant"),
        }
    }

    #[test]
    fn encode_response_error() {
        let resp = ControlResponse::Error {
            error: "command not found".to_string(),
        };
        let frame = encode_frame(&resp).unwrap();
        let json = &frame[4..];
        let parsed: ControlResponse = serde_json::from_slice(json).unwrap();
        match parsed {
            ControlResponse::Error { error } => {
                assert_eq!(error, "command not found");
            }
            _ => panic!("Expected Error variant"),
        }
    }

    #[test]
    fn encode_length_prefix_is_correct() {
        let cmd = ControlCommand::Ping;
        let frame = encode_frame(&cmd).unwrap();
        let len_prefix = u32::from_be_bytes([frame[0], frame[1], frame[2], frame[3]]);
        assert_eq!(len_prefix as usize, frame.len() - 4);
    }

    #[test]
    fn encode_empty_payload() {
        let cmd = ControlCommand::Shutdown;
        let frame = encode_frame(&cmd).unwrap();
        assert!(frame.len() > 4);
        let json = &frame[4..];
        let parsed: ControlCommand = serde_json::from_slice(json).unwrap();
        match parsed {
            ControlCommand::Shutdown => {}
            _ => panic!("Expected Shutdown variant"),
        }
    }

    // ── decode_frame tests ──

    #[test]
    fn decode_frame_complete() {
        let cmd = ControlCommand::List;
        let frame = encode_frame(&cmd).unwrap();
        let result = decode_frame(&frame);
        assert!(result.is_some());
        let (consumed, payload) = result.unwrap();
        assert_eq!(consumed, frame.len());
        let parsed: ControlCommand = serde_json::from_slice(&payload).unwrap();
        match parsed {
            ControlCommand::List => {}
            _ => panic!("Expected List variant"),
        }
    }

    #[test]
    fn decode_frame_empty_buffer() {
        assert!(decode_frame(&[]).is_none());
    }

    #[test]
    fn decode_frame_partial_header_1byte() {
        assert!(decode_frame(&[0x00]).is_none());
    }

    #[test]
    fn decode_frame_partial_header_3bytes() {
        assert!(decode_frame(&[0x00, 0x00, 0x00]).is_none());
    }

    #[test]
    fn decode_frame_header_only_no_body() {
        // 4-byte header saying 10 bytes, but no body
        let buf = [0x00, 0x00, 0x00, 0x0A];
        assert!(decode_frame(&buf).is_none());
    }

    #[test]
    fn decode_frame_partial_body() {
        let cmd = ControlCommand::List;
        let full_frame = encode_frame(&cmd).unwrap();
        // Truncate last byte
        let partial = &full_frame[..full_frame.len() - 1];
        assert!(decode_frame(partial).is_none());
    }

    #[test]
    fn decode_frame_extra_data_after_frame() {
        let cmd1 = ControlCommand::List;
        let cmd2 = ControlCommand::Shutdown;
        let frame1 = encode_frame(&cmd1).unwrap();
        let frame2 = encode_frame(&cmd2).unwrap();
        let mut combined = frame1.clone();
        combined.extend_from_slice(&frame2);

        let result = decode_frame(&combined);
        assert!(result.is_some());
        let (consumed, _) = result.unwrap();
        assert_eq!(consumed, frame1.len());
        // Remaining data should be frame2
        let remaining = &combined[consumed..];
        assert_eq!(remaining.len(), frame2.len());
    }

    #[test]
    fn roundtrip_all_command_variants() {
        let commands = vec![
            ControlCommand::List,
            ControlCommand::Spawn {
                cmd: "bash".to_string(),
                args: vec!["-c".to_string(), "echo hi".to_string()],
                env: Some(
                    [("PATH".to_string(), "/usr/bin".to_string())]
                        .into_iter()
                        .collect(),
                ),
                rows: None,
                cols: None,
                dir: None,
            },
            ControlCommand::SendKeys { id: "x".to_string(), keys: "enter".to_string() },
            ControlCommand::Kill { id: "y".to_string() },
            ControlCommand::Freeze { id: "z".to_string() },
            ControlCommand::Thaw { id: "w".to_string() },
            ControlCommand::Purge { id: "v".to_string() },
            ControlCommand::Restart { id: "u".to_string() },
            ControlCommand::Resize { id: "t".to_string(), rows: 40, cols: 120 },
            ControlCommand::Cat { id: "s".to_string() },
            ControlCommand::Snapshot { id: "r".to_string(), name: "snap1".to_string() },
            ControlCommand::ListSnapshots { id: "q".to_string() },
            ControlCommand::DeleteSnapshot { id: "p".to_string(), name: "snap1".to_string() },
            ControlCommand::Shutdown,
            ControlCommand::Ping,
        ];

        for cmd in commands {
            let frame = encode_frame(&cmd).unwrap();
            let (consumed, payload) = decode_frame(&frame).unwrap();
            assert_eq!(consumed, frame.len());
            let decoded: ControlCommand = serde_json::from_slice(&payload).unwrap();
            // Serialize both to JSON for comparison ( PartialEq not derived )
            assert_eq!(
                serde_json::to_value(&cmd).unwrap(),
                serde_json::to_value(&decoded).unwrap()
            );
        }
    }

    #[test]
    fn roundtrip_response_variants() {
        let responses = vec![
            ControlResponse::Ok { data: serde_json::Value::Null },
            ControlResponse::Ok { data: serde_json::json!({"commands": []}) },
            ControlResponse::Error { error: "not found".to_string() },
        ];

        for resp in responses {
            let frame = encode_frame(&resp).unwrap();
            let (consumed, payload) = decode_frame(&frame).unwrap();
            assert_eq!(consumed, frame.len());
            let decoded: ControlResponse = serde_json::from_slice(&payload).unwrap();
            assert_eq!(
                serde_json::to_value(&resp).unwrap(),
                serde_json::to_value(&decoded).unwrap()
            );
        }
    }

    #[test]
    fn decode_frame_zero_length_payload() {
        // Manually craft a frame with length=0
        let buf = [0x00, 0x00, 0x00, 0x00];
        let result = decode_frame(&buf);
        assert!(result.is_some());
        let (consumed, payload) = result.unwrap();
        assert_eq!(consumed, 4);
        assert!(payload.is_empty());
    }

    #[test]
    fn decode_multiple_consecutive_frames() {
        let cmds = vec![
            ControlCommand::List,
            ControlCommand::Ping,
            ControlCommand::Shutdown,
        ];
        let frames: Vec<u8> = cmds.iter().flat_map(|c| encode_frame(c).unwrap()).collect();

        let mut offset = 0;
        for expected_cmd in &cmds {
            let result = decode_frame(&frames[offset..]);
            assert!(result.is_some(), "Failed to decode frame at offset {}", offset);
            let (consumed, payload) = result.unwrap();
            let decoded: ControlCommand = serde_json::from_slice(&payload).unwrap();
            assert_eq!(
                serde_json::to_value(expected_cmd).unwrap(),
                serde_json::to_value(&decoded).unwrap()
            );
            offset += consumed;
        }
        assert_eq!(offset, frames.len());
    }
}
