//! Keybinding action dispatch for the interactive display loop.
//!
//! Extracted from display.rs for maintainability. Contains the central
//! action dispatch logic (switch tab, toggle log, kill, freeze/thaw, quit),
//! context menu action execution, spawn command handling, and the
//! result enums.

use std::sync::Arc;

use crate::interactive::{read_spawn_command, restore_raw_mode, ActionEffect};
use crate::process::manager::CommandManager;
use std::io::Write;

use super::mouse::base64_encode;

/// Result of [`dispatch_action_effect`], indicating what the caller should do
/// after processing a keybinding action.
#[derive(Debug)]
pub(crate) enum CommandLoopResult {
    /// Break out of the display loop.
    Break,
    /// Continue to the next iteration without rendering.
    Continue,
    /// Continue to the next iteration, but render first for responsiveness.
    RenderAndContinue,
}

/// Result of [`handle_spawn_command`].
#[derive(Debug)]
pub(crate) enum SpawnCommandResult {
    /// `restore_raw_mode()` failed — caller should break the display loop.
    ShouldBreak,
    /// Spawn succeeded. Contains the new command ID.
    Spawned(String),
    /// No action taken (user cancelled or empty input).
    NoOp,
}

// Centralizes keybinding action dispatch (switch tab, toggle log, kill,
// freeze/thaw, show help, quit) to avoid duplication between the single-byte
// and multi-byte keybinding match paths in the select! loop.
pub(crate) async fn dispatch_action_effect(
    manager: &Arc<CommandManager>,
    effect: ActionEffect,
    active_id: &mut Option<String>,
    log_scroll_offset: &mut usize,
    showing_log: &mut bool,
    showing_help: &mut bool,
    scrollback_offset: &mut usize,
) -> CommandLoopResult {
    match effect {
        ActionEffect::None => CommandLoopResult::Continue,
        ActionEffect::NextCommand | ActionEffect::PrevCommand => {
            let commands = manager.list();
            if commands.len() <= 1 {
                return CommandLoopResult::Continue;
            }
            let current = active_id
                .clone()
                .or_else(|| commands.first().map(|(id, _, _, _, _)| id.clone()));
            if let Some(ref cur) = current {
                let idx = commands
                    .iter()
                    .position(|(id, _, _, _, _)| id == cur)
                    .unwrap_or(0);
                let new_idx = if effect == ActionEffect::NextCommand {
                    (idx + 1) % commands.len()
                } else {
                    idx.checked_sub(1).unwrap_or(commands.len() - 1)
                };
                let (new_id, new_name, _, new_pid, _) = &commands[new_idx];
                *active_id = Some(new_id.clone());
                *scrollback_offset = 0;
                manager.logger().log(
                    "switch",
                    &format!("id={} name={} pid={}", new_id, new_name, new_pid),
                );
                CommandLoopResult::RenderAndContinue
            } else {
                CommandLoopResult::Continue
            }
        }
        ActionEffect::ToggleLog(show) => {
            *showing_log = show;
            *log_scroll_offset = 0;
            CommandLoopResult::Continue
        }
        ActionEffect::ShowHelp => {
            *showing_help = true;
            CommandLoopResult::RenderAndContinue
        }
        ActionEffect::KillCommand => {
            if let Some(id) = active_id.take() {
                manager
                    .logger()
                    .log("kill_keybinding", &format!("id={}", id));
                let _ = manager.kill(&id, None).await;
            }
            CommandLoopResult::Continue
        }
        ActionEffect::TogglePause => {
            if let Some(ref id) = active_id {
                if let Some(handle) = manager.get(id) {
                    if handle.is_alive() {
                        let _ = manager.freeze(id);
                        manager
                            .logger()
                            .log("freeze_keybinding", &format!("id={}", id));
                    } else {
                        let _ = manager.thaw(id);
                        manager
                            .logger()
                            .log("thaw_keybinding", &format!("id={}", id));
                    }
                }
            }
            CommandLoopResult::Continue
        }
        ActionEffect::Quit => CommandLoopResult::Break,
    }
}

/// Execute a context menu action (kill, purge, copy_id, restart).
/// Returns the new `active_id`: `None` means clear it, `Some(id)` means set to id.
pub(crate) async fn execute_context_menu_action(
    manager: &Arc<CommandManager>,
    action: &str,
    target_id: &str,
    active_id: &Option<String>,
) -> Option<String> {
    let target_string = target_id.to_string();
    match action {
        "kill" => {
            manager
                .logger()
                .log("ctx_kill", &format!("id={}", target_id));
            let _ = manager.kill(&target_string, None).await;
            if active_id.as_deref() == Some(target_id) {
                None
            } else {
                active_id.clone()
            }
        }
        "purge" => {
            manager
                .logger()
                .log("ctx_purge", &format!("id={}", target_id));
            let _ = manager.purge(&target_string);
            if active_id.as_deref() == Some(target_id) {
                None
            } else {
                active_id.clone()
            }
        }
        "copy_id" => {
            let encoded = base64_encode(target_id);
            let mut stdout = std::io::stdout();
            let _ = write!(stdout, "\x1b]52;c;{}\x07", encoded);
            let _ = stdout.flush();
            active_id.clone()
        }
        "restart" => {
            if let Some(h) = manager.get(&target_string) {
                let cmd = h.name.clone();
                let args = h.args.clone();
                drop(h);
                match manager
                    .spawn(
                        cmd,
                        args,
                        None,
                        None,
                        std::collections::HashMap::new(),
                        None,
                        None,
                        None,
                    )
                    .await
                {
                    Ok(new_id) => {
                        manager
                            .logger()
                            .log("ctx_restart", &format!("old={} new={}", target_id, new_id));
                        let _ = manager.purge(&target_string);
                        Some(new_id)
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "Context menu restart failed");
                        active_id.clone()
                    }
                }
            } else {
                active_id.clone()
            }
        }
        _ => active_id.clone(),
    }
}

/// Handle the SpawnCommand action: leave raw mode, read command string,
/// re-enter raw mode, and spawn via manager.
pub(crate) async fn handle_spawn_command(manager: &Arc<CommandManager>) -> SpawnCommandResult {
    let cmd_str = read_spawn_command();
    if !restore_raw_mode() {
        return SpawnCommandResult::ShouldBreak;
    }
    if let Some(cmd_str) = cmd_str {
        let parts: Vec<&str> = cmd_str.split_whitespace().collect();
        if !parts.is_empty() {
            let cmd = parts[0].to_string();
            let args = parts[1..].iter().map(|s| s.to_string()).collect();
            match manager
                .spawn(
                    cmd,
                    args,
                    None,
                    None,
                    std::collections::HashMap::new(),
                    None,
                    None,
                    None,
                )
                .await
            {
                Ok(id) => {
                    manager
                        .logger()
                        .log("spawn_terminal", &format!("id={} cmd={}", id, cmd_str));
                    return SpawnCommandResult::Spawned(id);
                }
                Err(e) => {
                    manager.logger().log(
                        "spawn_terminal_error",
                        &format!("error={} cmd={}", e, cmd_str),
                    );
                }
            }
        }
    }
    SpawnCommandResult::NoOp
}

// ── Send focus gained event to commands with ?1004h enabled ──
// When a command has enabled focus reporting, we send OSC 101 I
// to indicate the terminal gained focus (display mode entered).
pub(crate) async fn send_focus_event(manager: &Arc<CommandManager>, gained: bool) {
    let event = if gained {
        b"\x1b]101;i\x1b\\".to_vec()
    } else {
        b"\x1b]101;o\x1b\\".to_vec()
    };
    for entry in manager.list() {
        if let Some(handle) = manager.get(&entry.0) {
            if handle.focus_reporting_enabled().await {
                let _ = handle.send_bytes(event.clone()).await;
            }
        }
    }
}
