use super::null_sink::NullSink;

/// A sink that discards output (VTTY display is handled via broadcast channels).
/// This is an alias for NullSink — both discard all data identically.
pub type VttySink = NullSink;