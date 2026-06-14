use async_trait::async_trait;
use super::sink::Sink;

/// A sink that discards output (VTTY display is handled via broadcast channels).
/// This exists as a placeholder so "vtty" sink type can be resolved in config,
/// while actual terminal rendering goes through VttyOutput/BroadcastVttySink.
pub struct VttySink;

impl VttySink {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Sink for VttySink {
    async fn write(&mut self, _data: &[u8]) {
        // VTTY output is pushed via BroadcastVttySink directly;
        // this handle sink is a no-op.
    }

    async fn flush(&mut self) {}
}