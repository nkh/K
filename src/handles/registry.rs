use super::sink::Sink;
use std::collections::HashMap;

pub struct HandleRegistry {
    sinks: HashMap<String, Box<dyn Sink>>,
}

impl Default for HandleRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl HandleRegistry {
    pub fn new() -> Self {
        Self {
            sinks: HashMap::new(),
        }
    }

    pub fn add(&mut self, name: String, sink: Box<dyn Sink>) {
        self.sinks.insert(name, sink);
    }

    pub fn get(&self, name: &str) -> Option<&dyn Sink> {
        self.sinks.get(name).map(|s| s.as_ref())
    }

    pub fn list(&self) -> Vec<String> {
        self.sinks.keys().cloned().collect()
    }
}