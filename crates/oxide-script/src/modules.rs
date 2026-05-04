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
        for path in &self.search_paths {
            let mut file_path = path.clone();
            file_path.push(format!("{}.ox", name));
            
            if file_path.exists() {
                return fs::read_to_string(file_path)
                    .map_err(|e| format!("Failed to read module {}: {}", name, e));
            }
        }
        Err(format!("Module '{}' not found", name))
    }
}