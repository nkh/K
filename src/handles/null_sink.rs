use super::sink::Sink;

pub struct NullSink;

impl Sink for NullSink {
    fn write(&mut self, _data: &[u8]) -> std::io::Result<()> {
        Ok(())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}