//! Typed errors for the process management module.
//!
//! Replaces `anyhow::Error` in library code so callers can match on
//! specific error conditions. Implements `std::error::Error` for
//! automatic conversion to `anyhow::Error` at the binary boundary.

use std::fmt;

/// Typed error type for process management operations.
#[derive(Debug)]
pub enum ProcessError {
    /// Underlying I/O error.
    Io(std::io::Error),

    /// No command with the given ID in the manager.
    CommandNotFound(String),

    /// Duplicate command ID.
    CommandAlreadyExists(String),

    /// Failed to spawn a child process.
    SpawnFailed { cmd: String },

    /// Unknown sink type.
    UnknownSinkType(String),

    /// Duplicate sink name for a command.
    SinkAlreadyExists { name: String, command_id: String },

    /// OS signal delivery failed.
    SignalFailed {
        id: String,
        signal: String,
        code: i32,
    },

    /// Stdin channel was closed.
    ChannelClosed(String),

    /// Named buffer snapshot does not exist.
    SnapshotNotFound { name: String, command_id: String },

    /// Unix-only operation on non-Unix.
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

