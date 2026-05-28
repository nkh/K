use anyhow::{Context, Result};
use std::fs::OpenOptions;

use crate::config::schema::Config;

/// Daemonize the current process using the traditional double-fork technique.
///
/// This MUST be called BEFORE starting the tokio runtime. The `fork()` system
/// call only duplicates the calling thread; tokio's multi-threaded runtime
/// creates internal threads for I/O, timers, and blocking tasks that will NOT
/// exist in the child process, causing deadlocks and undefined behavior.
///
/// The double-fork pattern ensures:
/// 1. The parent returns immediately (first fork + parent exit)
/// 2. The intermediate child forks again and exits, preventing zombies
/// 3. The grandchild (daemon) is adopted by init/systemd, is a session leader,
///    has no controlling terminal, and runs in the background
pub fn daemonize(cfg: &Config) -> Result<()> {
    // Open log files BEFORE forking — if they fail, we can report the error
    let stdout_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&cfg.daemon.stdout_file)
        .with_context(|| format!("Failed to open stdout log file: {}", cfg.daemon.stdout_file))?;
    let stderr_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&cfg.daemon.stderr_file)
        .with_context(|| format!("Failed to open stderr log file: {}", cfg.daemon.stderr_file))?;

    // First fork — parent exits, child continues
    unsafe {
        let pid = libc::fork();
        if pid < 0 {
            anyhow::bail!("First fork failed: {}", std::io::Error::last_os_error());
        }
        if pid > 0 {
            // Parent process — exit cleanly
            std::process::exit(0);
        }
    }

    // Child process: create a new session (detach from controlling terminal)
    unsafe {
        if libc::setsid() < 0 {
            anyhow::bail!("setsid failed: {}", std::io::Error::last_os_error());
        }
    }

    // Second fork — ensures the process cannot acquire a controlling terminal
    unsafe {
        let pid = libc::fork();
        if pid < 0 {
            anyhow::bail!("Second fork failed: {}", std::io::Error::last_os_error());
        }
        if pid > 0 {
            // Intermediate child — exit
            std::process::exit(0);
        }
    }

    // Daemon process (grandchild): set working directory
    if let Err(e) = std::env::set_current_dir("/tmp") {
        anyhow::bail!("Failed to set working directory: {}", e);
    }

    // Redirect stdin/stdout/stderr
    use std::os::unix::io::AsRawFd;

    // Open /dev/null for stdin
    let devnull = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/null")
        .context("Failed to open /dev/null")?;

    unsafe {
        // Redirect stdin → /dev/null
        libc::dup2(devnull.as_raw_fd(), 0);
        // Redirect stdout → log file
        libc::dup2(stdout_file.as_raw_fd(), 1);
        // Redirect stderr → log file
        libc::dup2(stderr_file.as_raw_fd(), 2);
    }

    // Close the original file descriptors (now duplicated)
    drop(devnull);
    drop(stdout_file);
    drop(stderr_file);

    Ok(())
}
