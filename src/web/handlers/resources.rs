use axum::{
    extract::{Path, State},
    Json,
};
use serde_json::Value;

use crate::web::state::AppState;

/// GET /api/commands/:id/resources
/// Returns CPU, memory, and thread count for a running command by reading /proc/[pid]/stat.
/// Only works on Linux; returns zeros on other platforms.
pub async fn get_resources(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Json<Value> {
    let handle = match state.manager.get(&id) {
        Some(h) => h,
        None => {
            return Json(serde_json::json!({
                "status": "error",
                "data": null,
                "error": "Command not found"
            }));
        }
    };

    let pid = handle.pid;
    let is_alive = handle.is_alive();

    if !is_alive {
        return Json(serde_json::json!({
            "status": "ok",
            "data": {
                "pid": pid,
                "cpu_percent": null,
                "memory_mb": null,
                "threads": null,
                "alive": false,
            },
            "error": null
        }));
    }

    // Read /proc/[pid]/stat and /proc/[pid]/statm on Linux
    let result = read_proc_stats(pid);

    Json(serde_json::json!({
        "status": "ok",
        "data": {
            "pid": pid,
            "cpu_percent": result.cpu_percent,
            "memory_mb": result.memory_mb,
            "threads": result.threads,
            "alive": true,
        },
        "error": null
    }))
}

struct ProcStats {
    cpu_percent: Option<f64>,
    memory_mb: Option<f64>,
    threads: Option<u32>,
}

#[cfg(target_os = "linux")]
fn read_proc_stats(pid: u32) -> ProcStats {
    // Read /proc/[pid]/stat
    let stat_path = format!("/proc/{}/stat", pid);
    let stat_content = match std::fs::read_to_string(&stat_path) {
        Ok(c) => c,
        Err(_) => return ProcStats { cpu_percent: None, memory_mb: None, threads: None },
    };

    // Parse fields from /proc/[pid]/stat
    // Format: pid (comm) state ppid pgrp session tty_nr tpgid flags minflt cminflt majflt cmajflt utime stime ...
    // utime is field 14 (0-indexed: 13), stime is field 15 (0-indexed: 14)
    // The comm field can contain spaces and parens, so we find the last ')' and split from there.
    let fields: Vec<&str> = if let Some(idx) = stat_content.rfind(')') {
        stat_content[idx + 2..].split_whitespace().collect()
    } else {
        return ProcStats { cpu_percent: None, memory_mb: None, threads: None };
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
        None => return ProcStats {
            cpu_percent: Some(0.0),
            memory_mb: None,
            threads: Some(threads),
        },
    };

    // Calculate uptime from /proc/uptime
    let uptime_secs = match std::fs::read_to_string("/proc/uptime") {
        Ok(content) => content.split_whitespace().next()
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(0.0),
        Err(_) => return ProcStats {
            cpu_percent: Some(0.0),
            memory_mb: None,
            threads: Some(threads),
        },
    };

    // Read /proc/[pid]/stat for starttime (field 20 after ')', 0-indexed: 19)
    let starttime: u64 = fields.get(19).and_then(|v| v.parse().ok()).unwrap_or(0);
    let elapsed_ticks = (uptime_secs * hz as f64) as u64;
    let process_ticks = if elapsed_ticks > starttime { elapsed_ticks - starttime } else { 1 };

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
    if val > 0 { Some(val as u64) } else { None }
}

#[cfg(not(target_os = "linux"))]
fn read_proc_stats(_pid: u32) -> ProcStats {
    ProcStats { cpu_percent: None, memory_mb: None, threads: None }
}
