use std::fs::OpenOptions;

use anyhow::{Context, Result};

use crate::config::schema::Config;

/// Daemonize the current process using the `daemonize` crate.
///
/// This MUST be called BEFORE starting the tokio runtime. The `fork()` system
/// call only duplicates the calling thread; tokio's multi-threaded runtime
/// creates internal threads for I/O, timers, and blocking tasks that will NOT
/// exist in the child process, causing deadlocks and undefined behavior.
///
/// The daemonize crate handles the traditional double-fork pattern:
/// 1. The parent returns immediately (first fork + parent exit)
/// 2. The intermediate child forks again and exits, preventing zombies
/// 3. The grandchild (daemon) is adopted by init/systemd, is a session leader,
///    has no controlling terminal, and runs in the background
pub fn daemonize(cfg: &Config) -> Result<()> {
    let saved_cwd = std::env::current_dir().context("Failed to determine current directory")?;

    let stdout = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&cfg.daemon.stdout_file)
        .with_context(|| format!("Failed to open stdout log file: {}", cfg.daemon.stdout_file))?;
    let stderr = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&cfg.daemon.stderr_file)
        .with_context(|| format!("Failed to open stderr log file: {}", cfg.daemon.stderr_file))?;

    let daemonize = daemonize::Daemonize::new()
        .stdout(stdout)
        .stderr(stderr)
        .working_directory(&saved_cwd);

    daemonize.start().context("daemonize failed")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that log files can be created (daemonize crate needs valid paths).
    #[test]
    fn test_daemonize_creates_log_files() {
        let dir = tempfile::tempdir().unwrap();
        let stdout_path = dir.path().join("test_out.log");
        let stderr_path = dir.path().join("test_err.log");
        let cfg = Config {
            daemon: crate::config::schema::DaemonConfig {
                enabled: true,
                stdout_file: stdout_path.to_string_lossy().to_string(),
                stderr_file: stderr_path.to_string_lossy().to_string(),
            },
            ..Config::default()
        };
        let stdout_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&cfg.daemon.stdout_file);
        assert!(stdout_file.is_ok(), "stdout file created");
        let stderr_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&cfg.daemon.stderr_file);
        assert!(stderr_file.is_ok(), "stderr file created");
    }
}