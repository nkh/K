use std::sync::Arc;
use dashmap::DashMap;
use uuid::Uuid;

use crate::config::schema::Config;
use crate::logging::command_log::CommandLogger;
use super::handle::CommandHandle;
use super::spawner::ProcessSpawner;

pub type CommandId = String;

pub struct CommandManager {
    commands: Arc<DashMap<CommandId, CommandHandle>>,
    config: Config,
    logger: Arc<CommandLogger>,
}

impl CommandManager {
    pub fn new(config: Config) -> Self {
        let logger = Arc::new(
            CommandLogger::new(config.command_log.enabled, config.command_log.file.as_deref())
                .expect("Failed to initialize command logger")
        );
        Self {
            commands: Arc::new(DashMap::new()),
            config,
            logger,
        }
    }

    pub async fn spawn(&self, cmd: String, args: Vec<String>) -> anyhow::Result<CommandId> {
        let id = Uuid::new_v4().to_string();
        self.logger.log("spawn", &format!("id={} cmd={} args={:?}", id, cmd, args));

        let spawner = ProcessSpawner::new(&self.config.vtty);
        let handle = spawner.spawn(
            cmd,
            args,
            self.config.handles.clone(),
            &id,
        ).await?;

        self.commands.insert(id.clone(), handle);
        Ok(id)
    }

    pub fn get(&self, id: &CommandId) -> Option<dashmap::mapref::one::Ref<CommandId, CommandHandle>> {
        self.commands.get(id)
    }

    pub fn list(&self) -> Vec<(CommandId, String, u32)> {
        self.commands
            .iter()
            .map(|entry| {
                let handle = entry.value();
                (entry.key().clone(), handle.name.clone(), handle.pid)
            })
            .collect()
    }

    pub async fn kill(&self, id: &CommandId, _signal: Option<String>) -> anyhow::Result<()> {
        self.logger.log("kill", &format!("id={}", id));
        if let Some((_, handle)) = self.commands.remove(id) {
            handle.kill().await?;
        }
        Ok(())
    }

    pub fn logger(&self) -> Arc<CommandLogger> {
        self.logger.clone()
    }

    pub async fn send_keys(&self, id: &CommandId, keys: &str) -> anyhow::Result<()> {
        self.logger.log("send_keys", &format!("id={} keys={}", id, keys));
        if let Some(handle) = self.commands.get(id) {
            let bytes = encode_keys(keys);
            handle.send_bytes(bytes).await?;
            Ok(())
        } else {
            anyhow::bail!("Command {} not found", id)
        }
    }
}

pub fn encode_keys(keys: &str) -> Vec<u8> {
    let mut result = Vec::new();
    let mut chars = keys.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '<' {
            let mut seq = String::new();
            while let Some(&c) = chars.peek() {
                if c == '>' {
                    chars.next();
                    break;
                }
                seq.push(c);
                chars.next();
            }
            result.extend_from_slice(&encode_special_key(&seq));
        } else {
            result.push(ch as u8);
        }
    }

    result
}

fn encode_special_key(seq: &str) -> Vec<u8> {
    match seq {
        "Esc" => vec![0x1b],
        "Enter" | "Return" => vec![0x0d],
        "Tab" => vec![0x09],
        "Backspace" => vec![0x7f],
        "Delete" => vec![0x1b, b'[', b'3', b'~'],
        "Insert" => vec![0x1b, b'[', b'2', b'~'],
        "Home" => vec![0x1b, b'[', b'H'],
        "End" => vec![0x1b, b'[', b'F'],
        "PageUp" => vec![0x1b, b'[', b'5', b'~'],
        "PageDown" => vec![0x1b, b'[', b'6', b'~'],
        "Up" => vec![0x1b, b'[', b'A'],
        "Down" => vec![0x1b, b'[', b'B'],
        "Left" => vec![0x1b, b'[', b'D'],
        "Right" => vec![0x1b, b'[', b'C'],
        "F1" => vec![0x1b, b'[', b'1', b'1', b'~'],
        "F2" => vec![0x1b, b'[', b'1', b'2', b'~'],
        "F3" => vec![0x1b, b'[', b'1', b'3', b'~'],
        "F4" => vec![0x1b, b'[', b'1', b'4', b'~'],
        "F5" => vec![0x1b, b'[', b'1', b'5', b'~'],
        "F6" => vec![0x1b, b'[', b'1', b'7', b'~'],
        "F7" => vec![0x1b, b'[', b'1', b'8', b'~'],
        "F8" => vec![0x1b, b'[', b'1', b'9', b'~'],
        "F9" => vec![0x1b, b'[', b'2', b'0', b'~'],
        "F10" => vec![0x1b, b'[', b'2', b'1', b'~'],
        "F11" => vec![0x1b, b'[', b'2', b'3', b'~'],
        "F12" => vec![0x1b, b'[', b'2', b'4', b'~'],
        _ => {
            if let Some(rest) = seq.strip_prefix("C-") {
                if let Some(key) = rest.chars().next() {
                    let byte = if key.is_ascii_alphabetic() {
                        (key.to_ascii_uppercase() as u8) & 0x1f
                    } else {
                        match key {
                            '@' => 0x00,
                            '[' => 0x1b,
                            '\\' => 0x1c,
                            ']' => 0x1d,
                            '^' => 0x1e,
                            '_' => 0x1f,
                            '?' => 0x7f,
                            _ => key as u8,
                        }
                    };
                    return vec![byte];
                }
            }
            if let Some(rest) = seq.strip_prefix("A-") {
                if let Some(key) = rest.chars().next() {
                    return vec![0x1b, key as u8];
                }
            }
            seq.as_bytes().to_vec()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_plain_text() {
        assert_eq!(encode_keys("hello"), b"hello");
    }

    #[test]
    fn test_encode_ctrl_c() {
        assert_eq!(encode_keys("<C-c>"), vec![0x03]);
    }

    #[test]
    fn test_encode_ctrl_a() {
        assert_eq!(encode_keys("<C-a>"), vec![0x01]);
    }

    #[test]
    fn test_encode_enter() {
        assert_eq!(encode_keys("<Enter>"), vec![0x0d]);
    }

    #[test]
    fn test_encode_escape() {
        assert_eq!(encode_keys("<Esc>"), vec![0x1b]);
    }

    #[test]
    fn test_encode_arrow_keys() {
        assert_eq!(encode_keys("<Up>"), vec![0x1b, b'[', b'A']);
        assert_eq!(encode_keys("<Down>"), vec![0x1b, b'[', b'B']);
        assert_eq!(encode_keys("<Left>"), vec![0x1b, b'[', b'D']);
        assert_eq!(encode_keys("<Right>"), vec![0x1b, b'[', b'C']);
    }

    #[test]
    fn test_encode_mixed() {
        let result = encode_keys("hello<C-c>world");
        let mut expected = b"hello".to_vec();
        expected.push(0x03);
        expected.extend_from_slice(b"world");
        assert_eq!(result, expected);
    }

    #[test]
    fn test_encode_alt_key() {
        assert_eq!(encode_keys("<A-x>"), vec![0x1b, b'x']);
    }

    #[test]
    fn test_encode_delete() {
        assert_eq!(encode_keys("<Delete>"), vec![0x1b, b'[', b'3', b'~']);
    }
}
