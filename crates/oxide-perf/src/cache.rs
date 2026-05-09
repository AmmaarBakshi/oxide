use std::collections::HashMap;
use std::path::PathBuf;

pub struct CommandCache {
    paths: HashMap<String, PathBuf>,
}

impl CommandCache {
    pub fn new() -> Self {
        Self {
            paths: HashMap::new(),
        }
    }

    pub fn get(&self, cmd: &str) -> Option<PathBuf> {
        self.paths.get(cmd).cloned()
    }

    pub fn insert(&mut self, cmd: String, path: PathBuf) {
        self.paths.insert(cmd, path);
    }
}