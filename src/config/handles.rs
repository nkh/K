use serde::{Deserialize, Serialize};

/// A pre-configured output handle.
/// Handles can be attached to spawned commands to direct their output
/// to a file, VTTY, or null sink by name.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HandleConfig {
    /// Name of the handle (used as the identifier in the API).
    pub name: String,
    /// Sink type: "file", "vtty", or "null".
    pub sink: String,
    /// Path for file sinks. Supports {id} and {name} placeholders.
    pub path: Option<String>,
}
