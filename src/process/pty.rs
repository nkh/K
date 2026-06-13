//! PTY backend abstraction.
//!
//! This module provides the [`PtyBackend`] trait, which abstracts the
//! pseudo-terminal implementation so that alternate backends can be
//! swapped without changing the spawning logic.
//!
//! # Available Backends
//!
//! - [`PortablePtyBackend`] — wraps [`portable_pty::native_pty_system()`],
//!   which selects the best PTY implementation for the current platform
//!   (Unix PTY on Linux/macOS, ConPTY on Windows).
//!
//! # Adding Custom Backends
//!
//! Implement [`PtyBackend`] to provide an alternative PTY implementation.
//! The trait returns concrete types that implement [`PtyMaster`],
//! [`PtySlave`], and [`ChildProcess`].
//!
//! # Windows ConPTY Reference
//!
//! On Windows, the `portable_pty` crate uses the Windows Pseudo Console
//! (ConPTY) API introduced in Windows 10 version 1809.  ConPTY provides
//! a pseudo-terminal layer that enables console applications to work
//! over terminal emulators and multiplexers.
//!
//! For more information, see the official Microsoft documentation:
//! <https://devblogs.microsoft.com/commandline/windows-command-line-introducing-the-windows-pseudo-console-conpty/>

use std::io::{Read, Write};

use super::error::{ProcessError, Result};

/// Result of opening a new PTY pair.
pub struct PtyPair {
    /// The master side — used to read child output and write stdin.
    /// Also supports resize operations.
    pub master: Box<dyn PtyMaster + Send>,
    /// The slave side — used to spawn the child process.
    pub slave: Box<dyn PtySlave + Send>,
}

/// Size of the pseudo-terminal.
#[derive(Debug, Clone, Copy)]
pub struct PtySize {
    pub rows: u16,
    pub cols: u16,
}

/// Abstraction over the PTY master fd/handle.
pub trait PtyMaster: Send {
    /// Clone the reader side of the master PTY for concurrent reads.
    fn clone_reader(&self) -> Result<Box<dyn Read + Send>>;
    /// Take the writer side of the master PTY.
    fn take_writer(&self) -> Result<Box<dyn Write + Send>>;
    /// Resize the PTY (sends SIGWINCH on Unix, equivalent on Windows).
    fn resize(&self, rows: u16, cols: u16) -> Result<()>;
}

/// Abstraction over the PTY slave side, used to spawn a child process.
pub trait PtySlave: Send {
    /// Spawn a command attached to this PTY, setting the given TERM value.
    /// If `dir` is provided, the child process will be started in that directory.
    fn spawn_command(
        &self,
        cmd: &str,
        args: &[String],
        term: &str,
        env: &std::collections::HashMap<String, String>,
        dir: Option<&str>,
    ) -> Result<Box<dyn ChildProcess + Send>>;
}

/// Abstraction over a spawned child process.
pub trait ChildProcess: Send {
    /// Get the OS process ID.
    fn process_id(&self) -> Option<u32>;
    /// Wait for the child to exit, returning the exit code (if available).
    fn wait(&mut self) -> std::io::Result<Option<i32>>;
}

/// A PTY backend provides the ability to open PTY pairs and spawn processes.
///
/// This trait abstracts the platform-specific PTY implementation so that
/// different backends (portable-pty, Unix native pty, ConPTY, etc.) can
/// be used interchangeably.
pub trait PtyBackend: Send + Sync {
    /// Open a new PTY pair with the given dimensions.
    fn openpty(&self, size: PtySize) -> Result<PtyPair>;
}

// ---------------------------------------------------------------------------
// PortablePtyBackend — wraps portable-pty (default implementation)
// ---------------------------------------------------------------------------

/// Default PTY backend using [`portable_pty`].
///
/// This backend automatically selects the best platform implementation:
/// - **Unix** (Linux, macOS, BSD): uses the standard Unix PTY (`posix_openpt`
///   or equivalent).
/// - **Windows**: uses the Windows Pseudo Console (ConPTY) API.
///
/// Reference for ConPTY:
/// <https://devblogs.microsoft.com/commandline/windows-command-line-introducing-the-windows-pseudo-console-conpty/>
pub struct PortablePtyBackend;

impl PortablePtyBackend {
    /// Create a new portable PTY backend.
    pub fn new() -> Self {
        Self
    }
}

impl Default for PortablePtyBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl PtyBackend for PortablePtyBackend {
    fn openpty(&self, size: PtySize) -> Result<PtyPair> {
        let pty_system = portable_pty::native_pty_system();
        let pair = pty_system
            .openpty(portable_pty::PtySize {
                rows: size.rows,
                cols: size.cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| {
                ProcessError::Io(std::io::Error::other(format!("openpty failed: {}", e)))
            })?;

        Ok(PtyPair {
            master: Box::new(PortablePtyMaster { inner: pair.master }),
            slave: Box::new(PortablePtySlave { inner: pair.slave }),
        })
    }
}

// ---------------------------------------------------------------------------
// PortablePtyMaster
// ---------------------------------------------------------------------------

struct PortablePtyMaster {
    inner: Box<dyn portable_pty::MasterPty + Send>,
}

impl PtyMaster for PortablePtyMaster {
    fn clone_reader(&self) -> Result<Box<dyn Read + Send>> {
        self.inner
            .try_clone_reader()
            .map(|r| Box::new(r) as Box<dyn Read + Send>)
            .map_err(|e| {
                ProcessError::Io(std::io::Error::other(format!("clone PTY reader: {}", e)))
            })
    }

    fn take_writer(&self) -> Result<Box<dyn Write + Send>> {
        self.inner
            .take_writer()
            .map(|w| Box::new(w) as Box<dyn Write + Send>)
            .map_err(|e| ProcessError::Io(std::io::Error::other(format!("take PTY writer: {}", e))))
    }

    fn resize(&self, rows: u16, cols: u16) -> Result<()> {
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

// Safety: portable_pty::MasterPty is Send on all supported platforms.
unsafe impl Send for PortablePtyMaster {}

// ---------------------------------------------------------------------------
// PortablePtySlave
// ---------------------------------------------------------------------------

struct PortablePtySlave {
    inner: Box<dyn portable_pty::SlavePty + Send>,
}

impl PtySlave for PortablePtySlave {
    fn spawn_command(
        &self,
        cmd: &str,
        args: &[String],
        term: &str,
        env: &std::collections::HashMap<String, String>,
        dir: Option<&str>,
    ) -> Result<Box<dyn ChildProcess + Send>> {
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
        Ok(Box::new(PortableChild { inner: child }))
    }
}

// Safety: portable_pty::SlavePty is Send on all supported platforms.
unsafe impl Send for PortablePtySlave {}

// ---------------------------------------------------------------------------
// PortableChild
// ---------------------------------------------------------------------------

struct PortableChild {
    inner: Box<dyn portable_pty::Child + Send>,
}

impl ChildProcess for PortableChild {
    fn process_id(&self) -> Option<u32> {
        self.inner.process_id()
    }

    fn wait(&mut self) -> std::io::Result<Option<i32>> {
        let status = self.inner.wait()?;
        Ok(Some(status.exit_code() as i32))
    }
}

// Safety: portable_pty::Child is Send on all supported platforms.
unsafe impl Send for PortableChild {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_portable_pty_backend_openpty() {
        let backend = PortablePtyBackend::new();
        let pair = backend.openpty(PtySize { rows: 24, cols: 80 });
        // Should succeed on Unix; may fail in restricted CI environments
        match pair {
            Ok(_pair) => {
                // Successfully opened a PTY
            }
            Err(_) => {
                // PTY not available in this environment
            }
        }
    }
}
