use std::fs::File;
use std::process::{Command, Stdio};

/// Applies file redirection to a command before it runs
pub fn apply(process: &mut Command, outfile: &Option<String>) {
    if let Some(file_name) = outfile {
        match File::create(file_name) {
            Ok(file) => {
                // Route the process's standard output directly into the file!
                process.stdout(Stdio::from(file));
            }
            Err(e) => {
                eprintln!("oxide: failed to open '{}' for redirection: {}", file_name, e);
            }
        }
    }
}