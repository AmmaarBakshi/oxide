use std::fs::File;
use std::process::{Command, Stdio};

pub fn apply(process: &mut Command, outfile: &Option<String>) {
    if let Some(path) = outfile {
        match File::create(path) {
            Ok(file) => {
                process.stdout(Stdio::from(file));
            }
            Err(e) => {
                eprintln!("oxide: redirection error: {}", e);
            }
        }
    }
}