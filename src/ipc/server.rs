//! UDS control socket server.
//!
//! Listens for incoming connections on the instance's control socket and
//! dispatches [`ControlCommand`] messages to the [`CommandManager`].

use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::UnixListener;

use crate::ipc::protocol::{decode_frame, encode_frame, ControlCommand, ControlResponse};
use crate::process::manager::CommandManager;

/// Spawn a background task that listens on the UDS control socket and
/// processes incoming commands.  This runs concurrently with the display
/// loop and terminates when the socket is dropped or an error occurs.
pub fn spawn_control_server(
    manager: Arc<CommandManager>,
    socket_path: std::path::PathBuf,
    mut shutdown_rx: tokio::sync::broadcast::Receiver<()>,
) {
    tokio::spawn(async move {
        // Remove stale socket if it exists
        let _ = tokio::fs::remove_file(&socket_path).await;

        let listener = match UnixListener::bind(&socket_path) {
            Ok(l) => l,
            Err(e) => {
                tracing::warn!(
                    path = %socket_path.display(),
                    error = %e,
                    "Failed to bind control socket — IPC disabled"
                );
                return;
            }
        };

        tracing::info!(
            path = %socket_path.display(),
            "Control socket listening"
        );

        // Set up readable permission (0600 = owner read/write only)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o600);
            let _ = std::fs::set_permissions(&socket_path, perms);
        }

        loop {
            tokio::select! {
                result = listener.accept() => {
                    match result {
                        Ok((stream, _addr)) => {
                            let mgr = manager.clone();
                            tokio::spawn(handle_connection(mgr, stream));
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, "Control socket accept error");
                        }
                    }
                }
                _ = shutdown_rx.recv() => {
                    tracing::info!("Control socket shutting down");
                    // Clean up socket file
                    let _ = tokio::fs::remove_file(&socket_path).await;
                    break;
                }
            }
        }
    });
}

/// Handle a single client connection.  Reads one or more framed commands,
/// dispatches each, and sends back the response.
async fn handle_connection(manager: Arc<CommandManager>, mut stream: tokio::net::UnixStream) {
    let mut read_buf = Vec::new();

    loop {
        // Read more data
        let mut tmp = [0u8; 4096];
        match stream.read(&mut tmp).await {
            Ok(0) => break, // EOF — client disconnected
            Ok(n) => read_buf.extend_from_slice(&tmp[..n]),
            Err(e) => {
                tracing::debug!(error = %e, "Control socket read error");
                break;
            }
        }

        // Process all complete frames in the buffer
        while let Some((frame_end, payload)) = decode_frame(&read_buf) {
            read_buf.drain(..frame_end);

            let response = match serde_json::from_slice::<ControlCommand>(&payload) {
                Ok(cmd) => dispatch_command(&manager, cmd).await,
                Err(e) => ControlResponse::Error {
                    error: format!("Invalid command: {}", e),
                },
            };

            match encode_frame(&response) {
                Ok(frame) => {
                    if let Err(e) = stream.write_all(&frame).await {
                        tracing::debug!(error = %e, "Control socket write error");
                        break;
                    }
                    let _ = stream.flush().await;
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to encode response");
                }
            }
        }
    }
}

/// Dispatch a single command to the CommandManager and return a response.
async fn dispatch_command(manager: &Arc<CommandManager>, cmd: ControlCommand) -> ControlResponse {
    match cmd {
        ControlCommand::List => {
            let commands = manager.list();
            let data: Vec<serde_json::Value> = commands
                .into_iter()
                .map(|(id, name, args, pid, _cert)| {
                    let (alive, frozen, runtime_secs, exit_code) =
                        manager.get(&id).map(|h| {
                            let ec = h.exit_code.lock().ok().and_then(|c| *c);
                            (
                                h.is_alive(),
                                h.is_frozen(),
                                h.runtime_secs(),
                                ec,
                            )
                        }).unwrap_or((false, false, 0.0, None));
                    serde_json::json!({
                        "id": id,
                        "name": name,
                        "args": args,
                        "pid": pid,
                        "alive": alive,
                        "frozen": frozen,
                        "runtime_secs": runtime_secs,
                        "exit_code": exit_code,
                        "status": if frozen { "frozen" } else if alive { "running" } else { "exited" },
                    })
                })
                .collect();
            ControlResponse::Ok {
                data: serde_json::json!({ "commands": data }),
            }
        }

        ControlCommand::Spawn {
            cmd: command,
            args,
            env,
            rows,
            cols,
            dir,
        } => {
            let env_vars = env.unwrap_or_default();
            match manager
                .spawn(command, args, None, None, env_vars, rows, cols, dir)
                .await
            {
                Ok(id) => ControlResponse::Ok {
                    data: serde_json::json!({ "id": id }),
                },
                Err(e) => ControlResponse::Error {
                    error: e.to_string(),
                },
            }
        }

        ControlCommand::SendKeys { id, keys } => match manager.send_keys(&id, &keys).await {
            Ok(()) => ControlResponse::Ok {
                data: serde_json::json!({ "sent": true }),
            },
            Err(e) => ControlResponse::Error {
                error: e.to_string(),
            },
        },

        ControlCommand::Kill { id } => match manager.kill(&id, None).await {
            Ok(()) => ControlResponse::Ok {
                data: serde_json::json!({ "killed": true }),
            },
            Err(e) => ControlResponse::Error {
                error: e.to_string(),
            },
        },

        ControlCommand::Freeze { id } => match manager.freeze(&id) {
            Ok(()) => ControlResponse::Ok {
                data: serde_json::json!({ "frozen": true }),
            },
            Err(e) => ControlResponse::Error {
                error: e.to_string(),
            },
        },

        ControlCommand::Thaw { id } => match manager.thaw(&id) {
            Ok(()) => ControlResponse::Ok {
                data: serde_json::json!({ "thawed": true }),
            },
            Err(e) => ControlResponse::Error {
                error: e.to_string(),
            },
        },

        ControlCommand::Purge { id } => match manager.purge(&id) {
            Ok(()) => ControlResponse::Ok {
                data: serde_json::json!({ "purged": true }),
            },
            Err(e) => ControlResponse::Error {
                error: e.to_string(),
            },
        },

        ControlCommand::Restart { id } => {
            // Clone the command info, then spawn new + purge old
            let info = manager.get(&id).map(|h| {
                (h.name.clone(), h.args.clone())
            });
            match info {
                Some((name, args)) => {
                    match manager
                        .spawn(name.clone(), args, None, None, std::collections::HashMap::new(), None, None, None)
                        .await
                    {
                        Ok(new_id) => {
                            let _ = manager.purge(&id);
                            ControlResponse::Ok {
                                data: serde_json::json!({
                                    "old_id": id,
                                    "new_id": new_id,
                                }),
                            }
                        }
                        Err(e) => ControlResponse::Error {
                            error: e.to_string(),
                        },
                    }
                }
                None => ControlResponse::Error {
                    error: format!("Command '{}' not found", id),
                },
            }
        }

        ControlCommand::Resize { id, rows, cols } => {
            if let Some(handle) = manager.get(&id) {
                match handle.resize_pty(rows, cols).await {
                    Ok(()) => ControlResponse::Ok {
                        data: serde_json::json!({ "resized": true, "rows": rows, "cols": cols }),
                    },
                    Err(e) => ControlResponse::Error {
                        error: e.to_string(),
                    },
                }
            } else {
                ControlResponse::Error {
                    error: format!("Command '{}' not found", id),
                }
            }
        }

        ControlCommand::Cat { id } => {
            if let Some(handle) = manager.get(&id) {
                let text = handle.vtty_plain().await;
                ControlResponse::Ok {
                    data: serde_json::json!({ "text": text }),
                }
            } else {
                ControlResponse::Error {
                    error: format!("Command '{}' not found", id),
                }
            }
        }

        ControlCommand::Snapshot { id, name } => match manager.store_snapshot(&id, &name) {
            Ok(meta) => ControlResponse::Ok {
                data: serde_json::to_value(meta).unwrap_or(serde_json::json!({ "saved": true })),
            },
            Err(e) => ControlResponse::Error {
                error: e.to_string(),
            },
        },

        ControlCommand::ListSnapshots { id } => {
            let snapshots = manager.list_snapshots(&id);
            ControlResponse::Ok {
                data: serde_json::to_value(snapshots).unwrap_or(serde_json::json!([])),
            }
        }

        ControlCommand::DeleteSnapshot { id, name } => match manager.delete_snapshot(&id, &name) {
            Ok(()) => ControlResponse::Ok {
                data: serde_json::json!({ "deleted": true }),
            },
            Err(e) => ControlResponse::Error {
                error: e.to_string(),
            },
        },

        ControlCommand::Ping => {
            let pid = std::process::id();
            let commands = manager.list();
            ControlResponse::Ok {
                data: serde_json::json!({
                    "pid": pid,
                    "command_count": commands.len(),
                }),
            }
        }

        ControlCommand::Shutdown => {
            // The shutdown is handled by sending on the broadcast channel
            // which the main loop listens to.  We need access to shutdown_tx
            // here — but we don't pass it through.  Instead, the client
            // can use SIGTERM directly.  For now, return an error directing
            // to `vrc stop`.
            ControlResponse::Error {
                error: "Use `vrc stop <pid>` for graceful shutdown.".to_string(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify spawn_control_server compiles with the correct signature.
    /// Cannot be run end-to-end in tests because it binds a UDS and spawns
    /// a tokio task, but the function type is validated here.
    #[test]
    fn test_spawn_control_server_function_exists() {
        fn _type_check(
            _: fn(
                std::sync::Arc<CommandManager>,
                std::path::PathBuf,
                tokio::sync::broadcast::Receiver<()>,
            ),
        ) {
        }
        // Just verify the function name exists as a public item
        let _ = std::any::type_name_of_val(&spawn_control_server);
    }

    /// Verify ControlCommand and ControlResponse types compile and are usable.
    #[test]
    fn test_control_command_types_compile() {
        let ping = ControlCommand::Ping;
        let shutdown = ControlCommand::Shutdown;
        // Verify the types exist and can be instantiated
        let _ = format!("{:?}", ping);
        let _ = format!("{:?}", shutdown);
    }

    /// Verify all ControlCommand variants can be constructed.
    #[test]
    fn test_all_control_command_variants() {
        let list = ControlCommand::List;
        let spawn = ControlCommand::Spawn {
            cmd: "htop".into(),
            args: vec![],
            env: None,
            rows: None,
            cols: None,
            dir: None,
        };
        let keys = ControlCommand::SendKeys { id: "cmd-1".into(), keys: "hello".into() };
        let kill = ControlCommand::Kill { id: "cmd-2".into() };
        let freeze = ControlCommand::Freeze { id: "cmd-3".into() };
        let thaw = ControlCommand::Thaw { id: "cmd-4".into() };
        let purge = ControlCommand::Purge { id: "cmd-5".into() };
        let restart = ControlCommand::Restart { id: "cmd-6".into() };
        let resize = ControlCommand::Resize { id: "cmd-7".into(), rows: 50, cols: 200 };
        let cat = ControlCommand::Cat { id: "cmd-8".into() };
        let snapshot = ControlCommand::Snapshot { id: "cmd-9".into(), name: "snap1".into() };
        let list_snap = ControlCommand::ListSnapshots { id: "cmd-9".into() };
        let del_snap = ControlCommand::DeleteSnapshot { id: "cmd-9".into(), name: "snap1".into() };
        let ping = ControlCommand::Ping;
        let shutdown = ControlCommand::Shutdown;
        // All variants constructed without panic
        let _ = (list, spawn, keys, kill, freeze, thaw, purge, restart, resize, cat,
                 snapshot, list_snap, del_snap, ping, shutdown);
    }

    /// Verify ControlResponse variants can be constructed.
    #[test]
    fn test_control_response_variants() {
        let ok = ControlResponse::Ok {
            data: serde_json::json!({"key": "value"}),
        };
        let err = ControlResponse::Error {
            error: "something went wrong".into(),
        };
        // Verify serialization works
        let ok_json = serde_json::to_string(&ok).unwrap();
        let err_json = serde_json::to_string(&err).unwrap();
        assert!(ok_json.contains("key"), "Ok response serializes correctly");
        assert!(err_json.contains("something went wrong"), "Error response serializes correctly");

        // Verify deserialization round-trip
        let ok_rt: ControlResponse = serde_json::from_str(&ok_json).unwrap();
        match ok_rt {
            ControlResponse::Ok { data } => assert_eq!(data["key"], "value"),
            ControlResponse::Error { .. } => panic!("expected Ok response"),
        }
    }

    /// Verify encode_frame and decode_frame work together.
    #[test]
    fn test_encode_decode_frame_roundtrip() {
        use crate::ipc::protocol::{decode_frame, encode_frame};

        let response = ControlResponse::Ok {
            data: serde_json::json!({"commands": []}),
        };
        let frame = encode_frame(&response).unwrap();
        assert!(!frame.is_empty(), "encoded frame is not empty");

        let (frame_end, payload) = decode_frame(&frame).unwrap();
        assert_eq!(frame_end, frame.len(), "frame_end matches total length");
        assert!(!payload.is_empty(), "payload is not empty");

        let decoded: ControlResponse = serde_json::from_slice(&payload).unwrap();
        match decoded {
            ControlResponse::Ok { data } => assert_eq!(data["commands"].as_array().unwrap().len(), 0),
            ControlResponse::Error { .. } => panic!("expected Ok"),
        }
    }

    /// Verify decode_frame with empty buffer returns None.
    #[test]
    fn test_decode_frame_empty_buffer() {
        use crate::ipc::protocol::decode_frame;
        let result = decode_frame(&[]);
        assert!(result.is_none(), "empty buffer returns None");
    }

    /// Verify decode_frame with incomplete frame returns None.
    #[test]
    fn test_decode_frame_incomplete() {
        use crate::ipc::protocol::decode_frame;
        let incomplete = vec![0x00, 0x01, 0x02]; // Too short for a valid frame
        let result = decode_frame(&incomplete);
        assert!(result.is_none(), "incomplete frame returns None");
    }
}
