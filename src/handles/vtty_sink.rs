use super::sink::Sink;

pub struct VttySink;

impl VttySink {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl Sink for VttySink {
    async fn write(&mut self, _data: &[u8]) {
        // TODO: Feed data into VTTY emulator
    }

    async fn flush(&mut self) {}
}
