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
            // to `vrl stop`.
            ControlResponse::Error {
                error: "Use `vrl stop <pid>` for graceful shutdown.".to_string(),
            }
        }
    }
}
