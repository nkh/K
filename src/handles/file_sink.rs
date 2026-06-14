use super::sink::Sink;
use async_trait::async_trait;
use std::fs::OpenOptions;
use std::io::Write;

pub struct FileSink {
    #[allow(dead_code)]
    path: String,
    file: std::fs::File,
}

impl FileSink {
    pub fn new(path: &str) -> std::io::Result<Self> {
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        Ok(Self {
            path: path.to_string(),
            file,
        })
    }
}

#[async_trait]
impl Sink for FileSink {
    async fn write(&mut self, data: &[u8]) {
        let _ = self.file.write_all(data);
    }

    async fn flush(&mut self) {
        let _ = self.file.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_file_sink_write_and_flush() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.log");
        let mut sink = FileSink::new(path.to_str().unwrap()).unwrap();
        sink.write(b"hello\n").await;
        sink.flush().await;
        let contents = std::fs::read_to_string(path).unwrap();
        assert_eq!(contents, "hello\n");
    }

    #[tokio::test]
    async fn test_file_sink_multiple_writes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("multi.log");
        let mut sink = FileSink::new(path.to_str().unwrap()).unwrap();
        sink.write(b"line1\n").await;
        sink.write(b"line2\n").await;
        sink.write(b"line3\n").await;
        sink.flush().await;
        let contents = std::fs::read_to_string(path).unwrap();
        assert_eq!(contents, "line1\nline2\nline3\n");
    }

    #[test]
    fn test_file_sink_creates_new_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("new.log");
        assert!(!path.exists());
        let _sink = FileSink::new(path.to_str().unwrap()).unwrap();
        assert!(path.exists());
    }
}
