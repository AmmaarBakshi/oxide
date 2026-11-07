use std::path::PathBuf;

pub struct ModuleManager {
    #[allow(dead_code)]
    search_paths: Vec<PathBuf>,
}

impl Default for ModuleManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ModuleManager {
    pub fn new() -> Self {
        Self {
            search_paths: vec![PathBuf::from("./modules")],
        }
    }

    /// Reads the source of module `name`, adding the `.ox` extension when
    /// the name has none.
    pub fn load_module(&self, name: &str) -> Result<String, String> {
        let mut path = PathBuf::from(name);
        if path.extension().is_none() {
            path.set_extension("ox");
        }
        std::fs::read_to_string(&path).map_err(|e| format!("Module '{}' not found: {}", name, e))
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::ModuleManager;

    #[test]
    fn loads_a_module_adding_the_ox_extension() {
        let dir = std::env::temp_dir().join(format!("oxide_modules_{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("greet.ox"), "echo hi\n").unwrap();

        let name = dir.join("greet");
        let loaded = ModuleManager::new().load_module(name.to_str().unwrap());
        assert_eq!(loaded.as_deref(), Ok("echo hi\n"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_missing_module_is_an_error() {
        let err = ModuleManager::new()
            .load_module("no-such-module")
            .unwrap_err();
        assert!(
            err.starts_with("Module 'no-such-module' not found:"),
            "{}",
            err
        );
    }
}
