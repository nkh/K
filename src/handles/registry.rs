use std::collections::HashMap;
use super::sink::Sink;

pub struct HandleRegistry {
    sinks: HashMap<String, Box<dyn Sink>>,
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

    pub fn get(&self, name: &str) -> Option<&Box<dyn Sink>> {
        self.sinks.get(name)
    }

    pub fn list(&self) -> Vec<String> {
        self.sinks.keys().cloned().collect()
    }
}
