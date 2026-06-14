pub trait Sink: Send + Sync {
    fn write(&mut self, data: &[u8]) -> std::io::Result<()>;
    fn flush(&mut self) -> std::io::Result<()>;
}