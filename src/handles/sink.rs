use async_trait::async_trait;

#[async_trait]
pub trait Sink: Send + Sync {
    async fn write(&mut self, data: &[u8]);
    async fn flush(&mut self);
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test that Sink trait is object-safe by creating a boxed trait object.
    // The actual async tests for implementations are in their own files.

    #[test]
    fn test_sink_trait_is_object_safe() {
        let _box: Option<Box<dyn Sink>> = None;
        // This compiles only if Sink is object-safe
    }
}
