//! Typed errors for the process management module.
//!
//! Replaces `anyhow::Error` in library code so that callers can match on
//! specific error conditions instead of inspecting error strings.
//!
//! # Design
//!
//! Each variant represents a semantically distinct failure mode:
//!
//! | Variant               | Meaning                                      |
//! |-----------------------|----------------------------------------------|
//! | `Io`                  | Underlying I/O error (PTY, file, etc.)       |
//! | `CommandNotFound`     | No command with the given ID in the manager  |
//! | `CommandAlreadyExists`| Duplicate command ID registration            |
//! | `SpawnFailed`         | Child process could not be started           |
//! | `UnknownSinkType`     | Unsupported sink type string                 |
//! | `SinkAlreadyExists`   | Duplicate sink name for a command            |
//! | `SignalFailed`        | OS signal delivery returned non-zero         |
//! | `ChannelClosed`       | Stdin mpsc channel was dropped               |
//! | `SnapshotNotFound`    | Named buffer snapshot does not exist         |
//! | `PlatformNotSupported`| Unix-only operation on non-Unix              |
//!
//! # Compatibility
//!
//! `ProcessError` implements `std::error::Error`, so it converts
//! automatically to `anyhow::Error` at the binary boundary (web
//! handlers, CLI subcommands) without any extra glue.

use std::fmt;

/// Typed error type for process management operations.
#[derive(Debug)]
pub enum ProcessError {
    /// An I/O error occurred (PTY operations, file creation, etc.).
    Io(std::io::Error),

    /// A command with the given ID was not found in the manager.
    CommandNotFound(String),

    /// A command with the given ID is already registered.
    CommandAlreadyExists(String),

    /// Failed to spawn a child process.
    SpawnFailed {
        /// The command binary that was being spawned.
        cmd: String,
    },

    /// An unknown sink type was requested.
    UnknownSinkType(String),

    /// A sink with this name already exists for the given command.
    SinkAlreadyExists { name: String, command_id: String },

    /// An OS signal operation (SIGSTOP / SIGCONT / SIGKILL) failed.
    SignalFailed {
        /// The command ID the signal was sent to.
        id: String,
        /// Human-readable signal name (e.g. "SIGSTOP").
        signal: String,
        /// The return code from the kill syscall.
        code: i32,
    },

    /// The stdin channel to a command was closed unexpectedly.
    ChannelClosed(String),

    /// A named buffer snapshot was not found.
    SnapshotNotFound { name: String, command_id: String },

    /// Operation not supported on this platform.
    PlatformNotSupported(String),
}

impl fmt::Display for ProcessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {}", e),
            Self::CommandNotFound(id) => write!(f, "Command {} not found", id),
            Self::CommandAlreadyExists(id) => {
                write!(f, "Command {} is already registered", id)
            }
            Self::SpawnFailed { cmd } => {
                write!(f, "Failed to spawn process '{}'", cmd)
            }
            Self::UnknownSinkType(t) => {
                write!(f, "Unknown sink type '{}'. Supported: file, vtty, null", t)
            }
            Self::SinkAlreadyExists { name, command_id } => {
                write!(
                    f,
                    "Sink '{}' is already registered for command {}",
                    name, command_id
                )
            }
            Self::SignalFailed { id, signal, code } => {
                write!(
                    f,
                    "Failed to send {} to command {}: {} returned {}",
                    signal, id, signal, code
                )
            }
            Self::ChannelClosed(id) => {
                write!(f, "stdin channel closed for command {}", id)
            }
            Self::SnapshotNotFound { name, command_id } => {
                write!(
                    f,
                    "Snapshot '{}' not found for command {}",
                    name, command_id
                )
            }
            Self::PlatformNotSupported(op) => {
                write!(f, "{} is only supported on Unix-like systems", op)
            }
        }
    }
}

impl std::error::Error for ProcessError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for ProcessError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

/// Module-local `Result` alias using [`ProcessError`].
pub type Result<T> = std::result::Result<T, ProcessError>;

