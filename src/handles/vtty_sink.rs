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

impl Sink for VttySink {
    fn write(&mut self, _data: &[u8]) -> std::io::Result<()> {
        Ok(())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}