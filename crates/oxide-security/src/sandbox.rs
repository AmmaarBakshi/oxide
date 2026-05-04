use std::process::{Command, Stdio};
use std::path::PathBuf;

pub struct Sandbox {
    root_dir: PathBuf,
}

impl Sandbox {
    pub fn new(jail_path: &str) -> Self {
        let path = PathBuf::from(jail_path);
        // Ensure the jail directory exists
        if !path.exists() {
            std::fs::create_dir_all(&path).unwrap_or_default();
        }
        Self { root_dir: path }
    }

    /// Runs a command restricted to the jail directory
    pub fn run(&self, program: &str, args: &[String]) -> Result<i32, String> {
        let mut cmd = Command::new(program);
        
        // Get the host's PATH so the jail can find 'ls', 'echo', etc.
        let host_path = std::env::var("PATH").unwrap_or_default();

        cmd.args(args)
            .current_dir(&self.root_dir) // The "Jail": Keep them in this folder
            .env_clear()                 // Wipe sensitive vars (API keys, etc.)
            .env("PATH", host_path)      // Allow finding programs
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

        match cmd.spawn() {
            Ok(mut child) => {
                let status = child.wait().map_err(|e| e.to_string())?;
                Ok(status.code().unwrap_or(1))
            }
            Err(_) => Err(format!("Sandbox Error: Could not find or run '{}'", program)),
        }
    }
}