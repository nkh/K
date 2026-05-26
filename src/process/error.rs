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
    SinkAlreadyExists {
        name: String,
        command_id: String,
    },

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
    SnapshotNotFound {
        name: String,
        command_id: String,
    },

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
                write!(
                    f,
                    "Unknown sink type '{}'. Supported: file, vtty, null",
                    t
                )
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
            Self::SnapshotNotFound {
                name,
                command_id,
            } => {
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

// ---------------------------------------------------------------------------
// Result type alias for convenience
// ---------------------------------------------------------------------------

/// Module-local `Result` alias using [`ProcessError`].
pub type Result<T> = std::result::Result<T, ProcessError>;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_command_not_found() {
        let err = ProcessError::CommandNotFound("abc-123".into());
        assert_eq!(format!("{}", err), "Command abc-123 not found");
    }

    #[test]
    fn test_display_command_already_exists() {
        let err = ProcessError::CommandAlreadyExists("abc-123".into());
        assert_eq!(format!("{}", err), "Command abc-123 is already registered");
    }

    #[test]
    fn test_display_spawn_failed() {
        let err = ProcessError::SpawnFailed {
            cmd: "ls".into(),
        };
        assert_eq!(format!("{}", err), "Failed to spawn process 'ls'");
    }

    #[test]
    fn test_display_unknown_sink_type() {
        let err = ProcessError::UnknownSinkType("redis".into());
        assert!(format!("{}", err).contains("Unknown sink type 'redis'"));
    }

    #[test]
    fn test_display_sink_already_exists() {
        let err = ProcessError::SinkAlreadyExists {
            name: "stdout".into(),
            command_id: "c1".into(),
        };
        assert_eq!(
            format!("{}", err),
            "Sink 'stdout' is already registered for command c1"
        );
    }

    #[test]
    fn test_display_signal_failed() {
        let err = ProcessError::SignalFailed {
            id: "c1".into(),
            signal: "SIGSTOP".into(),
            code: -1,
        };
        assert_eq!(
            format!("{}", err),
            "Failed to send SIGSTOP to command c1: SIGSTOP returned -1"
        );
    }

    #[test]
    fn test_display_channel_closed() {
        let err = ProcessError::ChannelClosed("c1".into());
        assert_eq!(
            format!("{}", err),
            "stdin channel closed for command c1"
        );
    }

    #[test]
    fn test_display_snapshot_not_found() {
        let err = ProcessError::SnapshotNotFound {
            name: "snap1".into(),
            command_id: "c1".into(),
        };
        assert_eq!(
            format!("{}", err),
            "Snapshot 'snap1' not found for command c1"
        );
    }

    #[test]
    fn test_display_platform_not_supported() {
        let err = ProcessError::PlatformNotSupported("freeze".into());
        assert_eq!(
            format!("{}", err),
            "freeze is only supported on Unix-like systems"
        );
    }

    #[test]
    fn test_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err = ProcessError::from(io_err);
        assert!(matches!(err, ProcessError::Io(_)));
        assert!(format!("{}", err).contains("I/O error"));
    }

    #[test]
    fn test_io_error_source() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
        let err = ProcessError::Io(io_err);
        assert!(std::error::Error::source(&err).is_some());
    }

    #[test]
    fn test_non_io_error_no_source() {
        let err = ProcessError::CommandNotFound("x".into());
        assert!(std::error::Error::source(&err).is_none());
    }

    #[test]
    fn test_error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ProcessError>();
    }

    #[test]
    fn test_result_type_alias() {
        let r: Result<String> = Ok("hello".to_string());
        assert_eq!(r.unwrap(), "hello");
        let r: Result<String> = Err(ProcessError::CommandNotFound("x".into()));
        assert!(r.is_err());
    }
}
