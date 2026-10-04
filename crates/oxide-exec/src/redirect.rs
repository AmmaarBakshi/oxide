use std::fs::{File, OpenOptions};
use std::process::{Command, Stdio};

/// Opens a `>` / `>>` target. When `append` is true the file is opened in
/// append mode; otherwise it is truncated/created.
///
/// Shared by process spawning and by builtins, so both honor a redirect the
/// same way.
pub fn open_output(path: &str, append: bool) -> std::io::Result<File> {
    if append {
        OpenOptions::new().create(true).append(true).open(path)
    } else {
        File::create(path)
    }
}

/// Hooks up an output redirect on a child process.
pub fn apply_output(process: &mut Command, outfile: &Option<String>, append: bool) {
    if let Some(path) = outfile {
        match open_output(path, append) {
            Ok(file) => {
                process.stdout(Stdio::from(file));
            }
            Err(e) => {
                eprintln!("oxide: redirection error: {}: {}", path, e);
            }
        }
    }
}

/// Hooks up an input redirect (`< file`), feeding the file as the process stdin.
pub fn apply_input(process: &mut Command, infile: &Option<String>) {
    if let Some(path) = infile {
        match File::open(path) {
            Ok(file) => {
                process.stdin(Stdio::from(file));
            }
            Err(e) => {
                eprintln!("oxide: redirection error: {}: {}", path, e);
            }
        }
    }
}
