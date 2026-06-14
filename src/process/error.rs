//! Typed errors for the process management module.
//!
//! Replaces `anyhow::Error` in library code so callers can match on
//! specific error conditions. Implements `std::error::Error` for
//! automatic conversion to `anyhow::Error` at the binary boundary.

use thiserror::Error;

/// Typed error type for process management operations.
#[derive(Debug, Error)]
pub enum ProcessError {
    /// Underlying I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// No command with the given ID in the manager.
    #[error("Command {0} not found")]
    CommandNotFound(String),

    /// Duplicate command ID.
    #[error("Command {0} is already registered")]
    CommandAlreadyExists(String),

    /// Failed to spawn a child process.
    #[error("Failed to spawn process '{cmd}'")]
    SpawnFailed { cmd: String },

    /// Unknown sink type.
    #[error("Unknown sink type '{0}'. Supported: file, vtty, null")]
    UnknownSinkType(String),

    /// Duplicate sink name for a command.
    #[error("Sink '{name}' is already registered for command {command_id}")]
    SinkAlreadyExists { name: String, command_id: String },

    /// OS signal delivery failed.
    #[error("Failed to send {signal} to command {id}: {signal} returned {code}")]
    SignalFailed {
        id: String,
        signal: String,
        code: i32,
    },

    /// Stdin channel was closed.
    #[error("stdin channel closed for command {0}")]
    ChannelClosed(String),

    /// Named buffer snapshot does not exist.
    #[error("Snapshot '{name}' not found for command {command_id}")]
    SnapshotNotFound { name: String, command_id: String },

    /// Unix-only operation on non-Unix.
    #[error("{0} is only supported on Unix-like systems")]
    PlatformNotSupported(String),
}

/// Module-local `Result` alias using [`ProcessError`].
pub type Result<T> = std::result::Result<T, ProcessError>;