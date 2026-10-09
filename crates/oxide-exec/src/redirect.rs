use std::fs::{File, OpenOptions};
use std::io::Write;
use std::process::{Command, Stdio};

/// Opens a `>` / `>>` / `2>` / `2>>` target. When `append` is true the file
/// is opened in append mode; otherwise it is truncated/created.
///
/// Shared by process spawning and by builtins, so both honor a redirect the
/// same way. `/dev/null` works on every platform.
pub fn open_output(path: &str, append: bool) -> std::io::Result<File> {
    let path = null_device(path);
    if append {
        OpenOptions::new().create(true).append(true).open(path)
    } else {
        File::create(path)
    }
}

/// Maps `/dev/null`, the idiom scripts use to discard output, to `NUL` on
/// Windows, where no such path exists.
fn null_device(path: &str) -> &str {
    if cfg!(windows) && path == "/dev/null" {
        "NUL"
    } else {
        path
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

/// Hooks up a stderr redirect (`2> file`, `2>> file`) on a child process.
pub fn apply_error(process: &mut Command, errfile: &Option<String>, append: bool) {
    if let Some(path) = errfile {
        match open_output(path, append) {
            Ok(file) => {
                process.stderr(Stdio::from(file));
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
        match File::open(null_device(path)) {
            Ok(file) => {
                process.stdin(Stdio::from(file));
            }
            Err(e) => {
                eprintln!("oxide: redirection error: {}: {}", path, e);
            }
        }
    }
}

/// Reports an error about a command where that command's stderr would go,
/// so `missing-program 2>/dev/null` stays quiet the way it does elsewhere.
pub fn report(errfile: &Option<String>, append: bool, message: &str) {
    if let Some(path) = errfile {
        if let Ok(mut file) = open_output(path, append) {
            let _ = writeln!(file, "{}", message);
            return;
        }
    }
    eprintln!("{}", message);
}
