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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handles::null_sink::NullSink;

    #[test]
    fn test_registry_add_and_get() {
        let mut reg = HandleRegistry::new();
        reg.add("stdout".to_string(), Box::new(NullSink));
        assert!(reg.get("stdout").is_some());
        assert!(reg.get("stderr").is_none());
    }

    #[test]
    fn test_registry_list() {
        let mut reg = HandleRegistry::new();
        reg.add("a".to_string(), Box::new(NullSink));
        reg.add("b".to_string(), Box::new(NullSink));
        reg.add("c".to_string(), Box::new(NullSink));
        let names = reg.list();
        assert_eq!(names.len(), 3);
        assert!(names.contains(&"a".to_string()));
        assert!(names.contains(&"b".to_string()));
        assert!(names.contains(&"c".to_string()));
    }

    #[test]
    fn test_registry_overwrite() {
        let mut reg = HandleRegistry::new();
        reg.add("sink".to_string(), Box::new(NullSink));
        reg.add("sink".to_string(), Box::new(NullSink));
        assert_eq!(reg.list().len(), 1);
    }
}
