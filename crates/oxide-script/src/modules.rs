use std::path::PathBuf;
use std::fs;

pub struct ModuleManager {
    search_paths: Vec<PathBuf>,
}

impl ModuleManager {
    pub fn new() -> Self {
        Self { search_paths: vec![PathBuf::from("./modules")] }
    }

    pub fn load_module(&self, name: &str) -> Result<String, String> {
        // 1. Try with the extension
        let mut path = std::path::PathBuf::from(name);
        if path.extension().is_none() {
            path.set_extension("ox");
        }

        // 2. Debug: Print where we are actually looking
        println!("DEBUG: Searching for module at: {:?}", path.canonicalize().unwrap_or(path.clone()));

        std::fs::read_to_string(&path)
            .map_err(|e| format!("Module '{}' not found: {}", name, e))
    }
}