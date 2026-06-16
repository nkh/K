#![cfg(feature = "vrw")]

use axum::{
    extract::{Path, State},
    Json,
};
use serde_json::Value;

use crate::web::response::{api_err, api_ok};
use crate::web::state::AppState;

/// GET /api/commands/:id/resources
/// Returns CPU, memory, and thread count for a running command by reading /proc/[pid]/stat.
/// Only works on Linux; returns zeros on other platforms.
pub async fn get_resources(State(state): State<AppState>, Path(id): Path<String>) -> Json<Value> {
    let handle = match state.manager.get(&id) {
        Some(h) => h,
        None => {
            return api_err("Command not found");
        }
    };

    let pid = handle.pid;
    let is_alive = handle.is_alive();
    let is_frozen = handle.is_frozen();

    if !is_alive {
        return api_ok(serde_json::json!({
            "pid": pid,
            "cpu_percent": null,
            "memory_mb": null,
            "threads": null,
            "alive": false,
            "frozen": false,
        }));
    }

    // Frozen processes (SIGSTOP) consume no CPU — return 0 immediately
    if is_frozen {
        return api_ok(serde_json::json!({
            "pid": pid,
            "cpu_percent": Some(0.0),
            "memory_mb": null,
            "threads": null,
            "alive": true,
            "frozen": true,
        }));
    }

    // Read /proc/[pid]/stat and /proc/[pid]/statm on Linux
    let result = read_proc_stats(pid);

    api_ok(serde_json::json!({
        "pid": pid,
        "cpu_percent": result.cpu_percent,
        "memory_mb": result.memory_mb,
        "threads": result.threads,
        "alive": true,
        "frozen": false,
    }))
}

pub(crate) struct ProcStats {
    pub cpu_percent: Option<f64>,
    pub memory_mb: Option<f64>,
    pub threads: Option<u32>,
}

#[cfg(target_os = "linux")]
pub(crate) fn read_proc_stats(pid: u32) -> ProcStats {
    // Read /proc/[pid]/stat
    let stat_path = format!("/proc/{}/stat", pid);
    let stat_content = match std::fs::read_to_string(&stat_path) {
        Ok(c) => c,
        Err(_) => {
            return ProcStats {
                cpu_percent: None,
                memory_mb: None,
                threads: None,
            }
        }
    };

    // Parse fields from /proc/[pid]/stat
    // Format: pid (comm) state ppid pgrp session tty_nr tpgid flags minflt cminflt majflt cmajflt utime stime ...
    // utime is field 14 (0-indexed: 13), stime is field 15 (0-indexed: 14)
    // The comm field can contain spaces and parens, so we find the last ')' and split from there.
    let fields: Vec<&str> = if let Some(idx) = stat_content.rfind(')') {
        stat_content[idx + 2..].split_whitespace().collect()
    } else {
        return ProcStats {
            cpu_percent: None,
            memory_mb: None,
            threads: None,
        };
    };

    // After ')', the fields are: state ppid pgrp session tty_nr tpgid flags minflt cminflt majflt cmajflt utime stime cutime cstime priority nice num_threads ...
    // 0-indexed: state=0, ppid=1, ..., utime=11, stime=12, num_threads=17
    let utime: u64 = fields.get(11).and_then(|v| v.parse().ok()).unwrap_or(0);
    let stime: u64 = fields.get(12).and_then(|v| v.parse().ok()).unwrap_or(0);
    let threads: u32 = fields.get(17).and_then(|v| v.parse().ok()).unwrap_or(0);

    let total_ticks = utime + stime;

    // Read system boot time and jiffies from /proc/stat
    let hz = match get_clk_tck() {
        Some(v) => v,
        None => {
            return ProcStats {
                cpu_percent: Some(0.0),
                memory_mb: None,
                threads: Some(threads),
            }
        }
    };

    // Calculate uptime from /proc/uptime
    let uptime_secs = match std::fs::read_to_string("/proc/uptime") {
        Ok(content) => content
            .split_whitespace()
            .next()
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(0.0),
        Err(_) => {
            return ProcStats {
                cpu_percent: Some(0.0),
                memory_mb: None,
                threads: Some(threads),
            }
        }
    };

    // Read /proc/[pid]/stat for starttime (field 20 after ')', 0-indexed: 19)
    let starttime: u64 = fields.get(19).and_then(|v| v.parse().ok()).unwrap_or(0);
    let elapsed_ticks = (uptime_secs * hz as f64) as u64;
    let process_ticks = if elapsed_ticks > starttime {
        elapsed_ticks - starttime
    } else {
        1
    };

    // CPU percentage = (utime + stime) / process_ticks * 100
    // Cap at 100% for single thread, allow more for multi-thread
    let cpu_percent = if process_ticks > 0 {
        (total_ticks as f64 / process_ticks as f64) * 100.0
    } else {
        0.0
    };

    // Read memory from /proc/[pid]/statm
    // Format: size resident shared text lib data dt
    // resident is in pages (typically 4KB)
    let memory_mb = match std::fs::read_to_string(format!("/proc/{}/statm", pid)) {
        Ok(content) => {
            let parts: Vec<&str> = content.split_whitespace().collect();
            let resident_pages: u64 = parts.get(1).and_then(|v| v.parse().ok()).unwrap_or(0);
            // Get page size
            let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) as u64 };
            let bytes = resident_pages * page_size;
            Some(bytes as f64 / (1024.0 * 1024.0))
        }
        Err(_) => None,
    };

    ProcStats {
        cpu_percent: Some(cpu_percent.min(999.9)),
        memory_mb,
        threads: Some(threads),
    }
}

#[cfg(target_os = "linux")]
fn get_clk_tck() -> Option<u64> {
    let val = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
    if val > 0 {
        Some(val as u64)
    } else {
        None
    }
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn read_proc_stats(_pid: u32) -> ProcStats {
    ProcStats {
        cpu_percent: None,
        memory_mb: None,
        threads: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::schema::Config;
    use crate::process::manager::CommandManager;
    use crate::process::handle::CommandHandle;
    use crate::handles::registry::HandleRegistry;
    use crate::vtty::emulator::VttyEmulator;
    use crate::vtty::sink::VttyOutput;
    use crate::web::certs::CertificateStore;
    use crate::web::state::AppState;
    use std::sync::Arc;

    fn make_app_state() -> AppState {
        let mut config = Config::default();
        config.binary_name = "test".to_string();
        let manager = Arc::new(CommandManager::new(config));
        let (shutdown_tx, _) = tokio::sync::broadcast::channel::<()>(1);
        let cert_store = Arc::new(CertificateStore::new());
        let (vtty_tx, _) = tokio::sync::broadcast::channel::<(String, String)>(16);
        let (log_tx, _) = tokio::sync::broadcast::channel::<String>(16);
        AppState::new(manager, shutdown_tx, None, cert_store, vtty_tx, log_tx)
    }

    fn insert_mock_cmd(mgr: &CommandManager, id: &str, pid: u32) {
        let (stdin_tx, _stdin_rx) = tokio::sync::mpsc::channel::<crate::process::spawner::StdinMessage>(16);
        let (_exit_tx, exit_rx) = tokio::sync::oneshot::channel::<crate::process::spawner::ExitStatus>();
        let (watch_tx, watch_rx) = tokio::sync::watch::channel(false);
        std::mem::forget(watch_tx);
        let emu = VttyEmulator::new(24, 80, 1000);
        let handle = CommandHandle {
            id: id.to_string(), pid,
            name: format!("cmd-{}", id),
            args: vec![],
            emulator: std::sync::Arc::new(tokio::sync::RwLock::new(emu)),
            stdin_tx, _exit_rx: exit_rx,
            handle_registry: HandleRegistry::new(),
            certificate: None,
            exit_config: crate::config::schema::ExitConfig::default(),
            spawn_time: std::time::Instant::now(),
            pty_master: None,
            vtty_output: std::sync::Arc::new(VttyOutput::new()),
            exit_rx: watch_rx,
            exit_code: std::sync::Mutex::new(None),
            exit_time: std::sync::Mutex::new(None),
            frozen: std::sync::atomic::AtomicBool::new(false),
            prev_diff_snapshot: tokio::sync::Mutex::new(None),
        };
        mgr.commands_arc().insert(id.to_string(), handle);
    }

    #[tokio::test]
    async fn test_get_resources_not_found() {
        let state = make_app_state();
        let result = get_resources(State(state), Path("nonexistent".into())).await;
        assert_eq!(result.0["status"], "error");
        assert!(result.0["error"].as_str().unwrap().contains("not found"));
    }

    #[tokio::test]
    async fn test_get_resources_dead_pid() {
        let state = make_app_state();
        // Use a PID that doesn't exist so is_alive() returns false
        insert_mock_cmd(&state.manager, "cmd-1", 999999999);
        let result = get_resources(State(state), Path("cmd-1".into())).await;
        assert_eq!(result.0["status"], "ok");
        assert_eq!(result.0["data"]["pid"], 999999999);
        assert_eq!(result.0["data"]["alive"], false);
        assert!(result.0["data"]["cpu_percent"].is_null());
        assert!(result.0["data"]["memory_mb"].is_null());
        assert!(result.0["data"]["threads"].is_null());
    }

    #[tokio::test]
    async fn test_get_resources_self() {
        let state = make_app_state();
        // Insert a mock with our own PID so is_alive() returns true
        let own_pid = std::process::id();
        insert_mock_cmd(&state.manager, "cmd-self", own_pid);
        let result = get_resources(State(state), Path("cmd-self".into())).await;
        assert_eq!(result.0["status"], "ok");
        assert_eq!(result.0["data"]["pid"], own_pid);
        assert_eq!(result.0["data"]["alive"], true);
        // On Linux, we should get real stats; on other platforms, nulls
        #[cfg(target_os = "linux")]
        {
            assert!(result.0["data"]["threads"].is_number());
            assert!(result.0["data"]["threads"].as_u64().unwrap() > 0);
        }
    }

    #[test]
    fn test_read_proc_stats_invalid_pid() {
        let stats = read_proc_stats(999999999);
        assert!(stats.cpu_percent.is_none());
        assert!(stats.memory_mb.is_none());
        assert!(stats.threads.is_none());
    }
}