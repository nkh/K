use super::sink::Sink;
use async_trait::async_trait;

pub struct NullSink;

#[async_trait]
impl Sink for NullSink {
    async fn write(&mut self, _data: &[u8]) {}
    async fn flush(&mut self) {}
}
