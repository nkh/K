use anyhow::Result;
use serde_json;
use std::fs;
use std::path::PathBuf;
use sysinfo::{System, SystemExt};

use super::info::InstanceInfo;
use crate::config::schema::Config;

pub struct InstanceRegistry {
    dir: PathBuf,
}

impl InstanceRegistry {
    pub fn new() -> Result<Self> {
        let dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("vrunner")
            .join("instances");
        fs::create_dir_all(&dir)?;
        Ok(Self { dir })
    }

    pub fn register_current(&self, cfg: &Config) -> Result<()> {
        let pid = std::process::id();
        let info = InstanceInfo {
            pid,
            port: cfg.server.port,
            bind: cfg.server.bind.clone(),
            start_time: chrono::Utc::now(),
            daemon: cfg.daemon.enabled,
            display: cfg.display.enabled,
            command: None, // populated by caller if available
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

    pub fn list_instances(&self) -> Vec<InstanceInfo> {
        let mut system = System::new_all();
        system.refresh_all();

        let mut instances = Vec::new();
        if let Ok(entries) = fs::read_dir(&self.dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(stem) = path.file_stem() {
                    if let Ok(pid) = stem.to_string_lossy().parse::<u32>() {
                        // Check if process is still alive
                        if system.process(sysinfo::Pid::from(pid as usize)).is_some() {
                            if let Ok(content) = fs::read_to_string(&path) {
                                if let Ok(info) = serde_json::from_str::<InstanceInfo>(&content) {
                                    instances.push(info);
                                }
                            }
                        } else {
                            // Clean up stale pidfile
                            let _ = fs::remove_file(&path);
                        }
                    }
                }
            }
        }
        instances
    }

    pub fn print_list(&self) {
        let instances = self.list_instances();
        if instances.is_empty() {
            println!("No running vrunner instances.");
            return;
        }
        println!("{:<10} {:<8} {:<20} {:<10} {:<10} {}",
            "PID", "PORT", "BIND", "DAEMON", "DISPLAY", "COMMAND");
        for info in instances {
            println!("{:<10} {:<8} {:<20} {:<10} {:<10} {}",
                info.pid,
                info.port,
                info.bind,
                if info.daemon { "yes" } else { "no" },
                if info.display { "yes" } else { "no" },
                info.command.as_deref().unwrap_or("(idle)")
            );
        }
    }

    pub async fn stop_instance(&self, pid: u32) -> Result<()> {
        let instances = self.list_instances();
        let target = instances.iter().find(|i| i.pid == pid);
        match target {
            Some(info) => {
                let url = format!("http://{}:{}/api/shutdown", info.bind, info.port);
                let client = reqwest::Client::new();
                match client.post(&url).send().await {
                    Ok(resp) if resp.status().is_success() => {
                        println!("Instance {} stopped gracefully.", pid);
                    }
                    _ => {
                        println!("Failed to contact instance {}. You may need to run: kill {}", pid, pid);
                    }
                }
            }
            None => {
                println!("No running vrunner instance found with PID {}.", pid);
            }
        }
        Ok(())
    }
}
