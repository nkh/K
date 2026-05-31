use anyhow::Result;
use serde_json;
use std::fs;
use std::path::PathBuf;

use super::info::InstanceInfo;
use crate::config::schema::Config;

pub struct InstanceRegistry {
    dir: PathBuf,
}

impl InstanceRegistry {
    /// Create a new registry using the system data directory.
    ///
    /// On Linux this resolves to `~/.local/share/vrl/instances/`.
    /// The directory is created if it doesn't exist.
    pub fn new() -> Result<Self> {
        let dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("vrl")
            .join("instances");
        Self::with_dir(dir)
    }

    /// Create a registry backed by a specific directory.
    pub fn with_dir(dir: PathBuf) -> Result<Self> {
        fs::create_dir_all(&dir)?;
        Ok(Self { dir })
    }

    /// Register the current vrl instance.
    pub fn register_current(&self, cfg: &Config) -> Result<()> {
        let pid = std::process::id();
        let info = InstanceInfo {
            pid,
            start_time: chrono::Utc::now(),
            daemon: cfg.daemon.enabled,
            display: cfg.display.enabled,
        };
        let path = self.dir.join(format!("{}.json", pid));
        fs::write(&path, serde_json::to_string_pretty(&info)?)?;
        Ok(())
    }

    pub fn unregister_current(&self) -> Result<()> {
        let pid = std::process::id();
        let path = self.dir.join(format!("{}.json", pid));
        if path.exists() {
            fs::remove_file(&path)?;
        }
        Ok(())
    }

    /// Fast liveness check: read PID files, check /proc/<pid>/comm, return live instances.
    pub fn list_instances_fast(&self) -> Vec<InstanceInfo> {
        let mut instances = Vec::new();
        if let Ok(entries) = fs::read_dir(&self.dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(stem) = path.file_stem() {
                    if let Ok(pid) = stem.to_string_lossy().parse::<u32>() {
                        if Self::is_pid_vrl(pid) {
                            if let Ok(content) = fs::read_to_string(&path) {
                                if let Ok(info) = serde_json::from_str::<InstanceInfo>(&content)
                                {
                                    instances.push(info);
                                }
                            }
                        } else {
                            let _ = fs::remove_file(&path);
                        }
                    }
                }
            }
        }
        instances
    }

    /// Check if a PID is alive and belongs to a vrl process.
    fn is_pid_vrl(pid: u32) -> bool {
        #[cfg(target_os = "linux")]
        {
            let comm_path = std::path::Path::new("/proc").join(pid.to_string()).join("comm");
            if let Ok(comm) = fs::read_to_string(&comm_path) {
                let name = comm.trim().to_lowercase();
                name.contains("vrl")
            } else {
                false
            }
        }
        #[cfg(all(unix, not(target_os = "linux")))]
        {
            unsafe { libc::kill(pid as i32, 0) == 0 }
        }
        #[cfg(not(unix))]
        {
            true
        }
    }

    /// Full instance listing with /proc verification.
    pub fn list_instances(&self) -> Vec<InstanceInfo> {
        let mut instances = Vec::new();
        if let Ok(entries) = fs::read_dir(&self.dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(stem) = path.file_stem() {
                    if let Ok(pid) = stem.to_string_lossy().parse::<u32>() {
                        if Self::is_pid_vrl(pid) {
                            if let Ok(content) = fs::read_to_string(&path) {
                                if let Ok(info) =
                                    serde_json::from_str::<InstanceInfo>(&content)
                                {
                                    instances.push(info);
                                }
                            }
                        } else {
                            tracing::warn!(
                                pid,
                                "cleaning up stale instance registry entry (PID recycled)"
                            );
                            let _ = fs::remove_file(&path);
                        }
                    }
                }
            }
        }
        instances
    }
}
