use super::sink::Sink;
use async_trait::async_trait;

pub struct NullSink;

#[async_trait]
impl Sink for NullSink {
    async fn write(&mut self, _data: &[u8]) {}
    async fn flush(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_null_sink_write_does_nothing() {
        let mut sink = NullSink;
        sink.write(b"hello").await;
        // No panic, no error — just verifies it doesn't crash
    }

    #[tokio::test]
    async fn test_null_sink_flush_does_nothing() {
        let mut sink = NullSink;
        sink.flush().await;
    }

    #[tokio::test]
    async fn test_null_sink_write_empty() {
        let mut sink = NullSink;
        sink.write(b"").await;
    }

    #[tokio::test]
    async fn test_null_sink_write_large() {
        let mut sink = NullSink;
        let data = vec![0u8; 65536];
        sink.write(&data).await;
    }
}
