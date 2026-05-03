use std::collections::HashSet;

pub struct PermissionManager {
    blocked_commands: HashSet<String>,
    protected_paths: Vec<String>,
}

impl PermissionManager {
    pub fn new() -> Self {
        let mut blocked = HashSet::new();
        blocked.insert("format".to_string()); // Never allow formatting drives!
        
        Self {
            blocked_commands: blocked,
            protected_paths: vec!["C:\\Windows".to_string(), "/etc".to_string()],
        }
    }

    pub fn is_allowed(&self, program: &str, args: &[String]) -> Result<(), String> {
        // 1. Check if command is blacklisted
        if self.blocked_commands.contains(program) {
            return Err(format!("Security Error: Command '{}' is banned.", program));
        }

        // 2. Check if arguments try to touch system folders
        for arg in args {
            for protected in &self.protected_paths {
                if arg.contains(protected) && (program == "rm" || program == "del") {
                    return Err(format!("Security Warning: Unauthorized access to {}", protected));
                }
            }
        }

        Ok(())
    }
}