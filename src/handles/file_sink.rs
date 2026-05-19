use super::sink::Sink;
use std::fs::OpenOptions;
use std::io::Write;

pub struct FileSink {
    path: String,
    file: std::fs::File,
}

impl FileSink {
    pub fn new(path: &str) -> anyhow::Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        Ok(Self {
            path: path.to_string(),
            file,
        })
    }
}

#[async_trait::async_trait]
impl Sink for FileSink {
    async fn write(&mut self, data: &[u8]) {
        let _ = self.file.write_all(data);
    }

    async fn flush(&mut self) {
        let _ = self.file.flush();
    }
}
