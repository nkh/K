use super::sink::Sink;
use async_trait::async_trait;

/// A sink that merges output into the VTTY stream.
/// Currently a placeholder - in a full implementation this would
/// write to a secondary channel that the emulator reads from.
pub struct VttySink;

impl Default for VttySink {
    fn default() -> Self {
        Self::new()
    }
}

impl VttySink {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Sink for VttySink {
    async fn write(&mut self, _data: &[u8]) {
        // TODO: In a full implementation, this would write to a channel
        // that the VTTY emulator reads from, merging the output into
        // the main terminal stream.
    }

    async fn flush(&mut self) {}
}
