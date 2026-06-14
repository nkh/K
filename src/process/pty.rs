//! Pseudo-terminal support via [`portable_pty`].

use std::io::{Read, Write};

use super::error::{ProcessError, Result};

/// Result of opening a new PTY pair.
pub struct PtyPair {
    /// The master side — used to read child output and write stdin.
    /// Also supports resize operations.
    pub master: PtyMaster,
    /// The slave side — used to spawn the child process.
    pub slave: PtySlave,
}

/// Size of the pseudo-terminal.
#[derive(Debug, Clone, Copy)]
pub struct PtySize {
    pub rows: u16,
    pub cols: u16,
}

/// Open a new PTY pair with the given dimensions.
///
/// Uses [`portable_pty::native_pty_system()`] which selects the best
/// platform implementation (Unix PTY on Linux/macOS, ConPTY on Windows).
pub fn openpty(rows: u16, cols: u16) -> Result<PtyPair> {
    let pty_system = portable_pty::native_pty_system();
    let pair = pty_system
        .openpty(portable_pty::PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| ProcessError::Io(std::io::Error::other(format!("openpty failed: {}", e))))?;

    Ok(PtyPair {
        master: PtyMaster { inner: pair.master },
        slave: PtySlave { inner: pair.slave },
    })
}

// ---------------------------------------------------------------------------
// PtyMaster
// ---------------------------------------------------------------------------

pub struct PtyMaster {
    inner: Box<dyn portable_pty::MasterPty + Send>,
}

impl PtyMaster {
    /// Clone the reader side of the master PTY for concurrent reads.
    pub fn clone_reader(&self) -> Result<Box<dyn Read + Send>> {
        self.inner
            .try_clone_reader()
            .map(|r| Box::new(r) as Box<dyn Read + Send>)
            .map_err(|e| {
                ProcessError::Io(std::io::Error::other(format!("clone PTY reader: {}", e)))
            })
    }

    /// Take the writer side of the master PTY.
    pub fn take_writer(&self) -> Result<Box<dyn Write + Send>> {
        self.inner
            .take_writer()
            .map(|w| Box::new(w) as Box<dyn Write + Send>)
            .map_err(|e| ProcessError::Io(std::io::Error::other(format!("take PTY writer: {}", e))))
    }

    /// Resize the PTY (sends SIGWINCH on Unix, equivalent on Windows).
    pub fn resize(&self, rows: u16, cols: u16) -> Result<()> {
        self.inner
            .resize(portable_pty::PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| {
                ProcessError::Io(std::io::Error::other(format!("PTY resize failed: {}", e)))
            })
    }
}

// ---------------------------------------------------------------------------
// PtySlave
// ---------------------------------------------------------------------------

pub struct PtySlave {
    inner: Box<dyn portable_pty::SlavePty + Send>,
}

impl PtySlave {
    /// Spawn a command attached to this PTY, setting the given TERM value.
    /// If `dir` is provided, the child process will be started in that directory.
    pub fn spawn_command(
        &self,
        cmd: &str,
        args: &[String],
        term: &str,
        env: &std::collections::HashMap<String, String>,
        dir: Option<&str>,
    ) -> Result<ChildProcess> {
        let mut cmd_builder = portable_pty::CommandBuilder::new(cmd);
        for arg in args {
            cmd_builder.arg(arg);
        }
        // Set TERM and environment variables
        cmd_builder.env("TERM", term);
        for (key, value) in env {
            cmd_builder.env(key, value);
        }
        // Set working directory — ALWAYS explicitly.
        // portable-pty 0.8.x falls back to $HOME when cwd is None
        // (cmdbuilder.rs:as_command → unwrap_or(home)), so we must
        // supply the actual current directory ourselves.
        let cwd: std::path::PathBuf = match dir {
            Some(d) => std::path::PathBuf::from(d),
            None => std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
        };
        cmd_builder.cwd(&cwd);
        let child =
            self.inner
                .spawn_command(cmd_builder)
                .map_err(|_| ProcessError::SpawnFailed {
                    cmd: cmd.to_string(),
                })?;
        Ok(ChildProcess { inner: child })
    }
}

// ---------------------------------------------------------------------------
// ChildProcess
// ---------------------------------------------------------------------------

pub struct ChildProcess {
    inner: Box<dyn portable_pty::Child + Send>,
}

impl ChildProcess {
    /// Get the OS process ID.
    pub fn process_id(&self) -> Option<u32> {
        self.inner.process_id()
    }

    /// Wait for the child to exit, returning the exit code (if available).
    pub fn wait(&mut self) -> std::io::Result<Option<i32>> {
        let status = self.inner.wait()?;
        Ok(Some(status.exit_code() as i32))
    }
}

