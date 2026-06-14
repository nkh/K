use anyhow::Result;
use serde_json;
use std::fs;
use std::path::PathBuf;

#[cfg(feature = "vrw")]
use sysinfo::{ProcessExt, SystemExt};

#[cfg(feature = "vrw")]
use crate::cli::commands::common::format_instance_list;

use super::info::InstanceInfo;
use crate::config::schema::Config;

pub struct InstanceRegistry {
    dir: PathBuf,
}

// Data directory name: "vrc" for vrc, "vrw" for vrw.
#[cfg(feature = "vrw")]
const DATA_DIR: &str = "vrw";
#[cfg(not(feature = "vrw"))]
const DATA_DIR: &str = "vrc";

// Process name for liveness check
#[cfg(feature = "vrw")]
const PROCESS_NAME: &str = "vrw";
#[cfg(not(feature = "vrw"))]
const PROCESS_NAME: &str = "vrc";

impl InstanceRegistry {
    /// Create a new registry using the system data directory.
    pub fn new() -> Result<Self> {
        let dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join(DATA_DIR)
            .join("instances");
        Self::with_dir(dir)
    }

    /// Create a registry backed by a specific directory.
    pub fn with_dir(dir: PathBuf) -> Result<Self> {
        fs::create_dir_all(&dir)?;
        Ok(Self { dir })
    }

    /// Register the current instance.
    pub fn register_current(&self, cfg: &Config) -> Result<()> {
        let pid = std::process::id();

        #[cfg(feature = "vrw")]
        let info = InstanceInfo {
            pid,
            port: cfg.server.port,
            bind: cfg.server.bind.clone(),
            name: cfg.server.name.clone(),
            start_time: chrono::Utc::now(),
            daemon: cfg.daemon.enabled,
            display: cfg.display.enabled,
            command: None,
        };

        #[cfg(not(feature = "vrw"))]
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

    /// List running instances.
    /// Uses sysinfo on vrw, /proc on vrc.
    pub fn list_instances(&self) -> Vec<InstanceInfo> {
        let pid_entries = self.scan_pid_files();
        #[cfg(feature = "vrw")]
        {
            self.filter_alive_sysinfo(pid_entries)
        }
        #[cfg(not(feature = "vrw"))]
        {
            self.filter_alive_proc(pid_entries)
        }
    }

    /// Scan the instance directory for PID files.
    ///
    /// Returns `(pid, file_content)` pairs for every JSON file whose filename
    /// parses as a u32.  Stale files (whose PID no longer exists) are cleaned
    /// up by the caller.
    fn scan_pid_files(&self) -> Vec<(u32, String)> {
        let mut entries = Vec::new();
        if let Ok(dir_entries) = fs::read_dir(&self.dir) {
            for entry in dir_entries.flatten() {
                let path = entry.path();
                if let Some(stem) = path.file_stem() {
                    if let Ok(pid) = stem.to_string_lossy().parse::<u32>() {
                        if let Ok(content) = fs::read_to_string(&path) {
                            entries.push((pid, content));
                        }
                    }
                }
            }
        }
        entries
    }

    /// Filter PID entries to only those whose process is alive (vrc: /proc or kill(0)).
    #[cfg(not(feature = "vrw"))]
    fn filter_alive_proc(&self, entries: Vec<(u32, String)>) -> Vec<InstanceInfo> {
        let mut instances = Vec::new();
        for (pid, content) in entries {
            if Self::is_pid_alive(pid) {
                if let Ok(info) = serde_json::from_str::<InstanceInfo>(&content) {
                    instances.push(info);
                }
            } else {
                let path = self.dir.join(format!("{}.json", pid));
                let _ = fs::remove_file(path);
            }
        }
        instances
    }

    /// Filter PID entries to only those whose process is alive (vrw: sysinfo).
    #[cfg(feature = "vrw")]
    fn filter_alive_sysinfo(&self, entries: Vec<(u32, String)>) -> Vec<InstanceInfo> {
        let mut system = sysinfo::System::new();
        system.refresh_all();

        let mut instances = Vec::new();
        for (pid, content) in entries {
            match system.process(sysinfo::Pid::from(pid as usize)) {
                Some(proc) => {
                    let name = proc.name().to_lowercase();
                    if name.contains(PROCESS_NAME) {
                        if let Ok(info) = serde_json::from_str::<InstanceInfo>(&content) {
                            instances.push(info);
                        }
                    } else {
                        tracing::warn!(
                            pid,
                            actual_name = %name,
                            "cleaning up stale instance registry entry (PID recycled)"
                        );
                        let path = self.dir.join(format!("{}.json", pid));
                        let _ = fs::remove_file(path);
                    }
                }
                None => {
                    let path = self.dir.join(format!("{}.json", pid));
                    let _ = fs::remove_file(path);
                }
            }
        }
        instances
    }

    /// Check if a PID is alive and belongs to our process.
    #[cfg(not(feature = "vrw"))]
    fn is_pid_alive(pid: u32) -> bool {
        #[cfg(target_os = "linux")]
        {
            let comm_path = std::path::Path::new("/proc").join(pid.to_string()).join("comm");
            if let Ok(comm) = fs::read_to_string(&comm_path) {
                let name = comm.trim().to_lowercase();
                name.contains(PROCESS_NAME)
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

    /// Print instance list (vrw only, used by registry directly).
    #[cfg(feature = "vrw")]
    pub fn print_list(&self) {
        let instances = self.list_instances();
        if instances.is_empty() {
            println!("No running vrw instances.");
            return;
        }
        print!("{}", format_instance_list(&instances));
    }

    /// Stop an instance via HTTP (vrw only).
    #[cfg(feature = "vrw")]
    pub async fn stop_instance(&self, pid: u32) -> Result<()> {
        let instances = self.list_instances();
        let target = instances.iter().find(|i| i.pid == pid);
        match target {
            Some(info) => {
                let url = format!("http://{}:{}/api/shutdown", info.bind, info.port);
                let client = reqwest::Client::builder()
                    .connect_timeout(std::time::Duration::from_secs(3))
                    .timeout(std::time::Duration::from_secs(5))
                    .build()?;
                match client.post(&url).send().await {
                    Ok(resp) if resp.status().is_success() => {
                        println!("Instance {} stopped gracefully.", pid);
                    }
                    Ok(resp) => {
                        let status = resp.status();
                        tracing::error!("Shutdown request returned HTTP {}", status);
                        println!(
                            "Failed to stop instance {} (HTTP {}). You may need to run: kill {}",
                            pid, status, pid
                        );
                    }
                    Err(e) => {
                        tracing::error!(pid = pid, url = %url, error = %e, "Failed to contact instance");
                        println!("Failed to contact instance {} ({}). Is the web server running? Try: kill {}", pid, e, pid);
                    }
                }
            }
            None => {
                println!("No running vrw instance found with PID {}.", pid);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_scan_pid_files_with_stale_file() {
        let dir = tempfile::tempdir().unwrap();
        // Create a fake PID file for a process that doesn't exist
        let pid = 999999999u32;
        let info_json = serde_json::json!({"pid": pid, "start_time": chrono::Utc::now(), "daemon": false, "display": false});
        std::fs::write(dir.path().join(format!("{}.json", pid)), info_json.to_string()).unwrap();

        let reg = InstanceRegistry::with_dir(dir.path().to_path_buf()).unwrap();
        let entries = reg.scan_pid_files();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].0, pid);

        // list_instances should filter out stale files
        let instances = reg.list_instances();
        assert!(instances.is_empty());

        // Stale file should be cleaned up
        assert!(!dir.path().join(format!("{}.json", pid)).exists());
    }

    #[test]
    fn test_registry_scan_pid_files_with_invalid_filename() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("not-a-pid.txt"), "garbage").unwrap();
        std::fs::write(dir.path().join("abc.json"), "{}").unwrap();

        let reg = InstanceRegistry::with_dir(dir.path().to_path_buf()).unwrap();
        let entries = reg.scan_pid_files();
        assert!(entries.is_empty());
    }
}
