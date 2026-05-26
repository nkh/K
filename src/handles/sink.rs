use async_trait::async_trait;

#[async_trait]
pub trait Sink: Send + Sync {
    async fn write(&mut self, data: &[u8]);
    async fn flush(&mut self);
}
