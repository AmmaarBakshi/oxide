use crate::redirect;
use std::process::{Child, Command, ExitStatus};

/// The shell-visible status of a finished process.
///
/// A process killed by a signal has no exit code; like other shells we
/// report it as 128 plus the signal number (so SIGKILL is 137).
pub fn status_code(status: ExitStatus) -> i32 {
    if let Some(code) = status.code() {
        return code;
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(signal) = status.signal() {
            return 128 + signal;
        }
    }
    1
}

/// Runs a single command and returns its exit code
pub fn spawn_single(
    program: &str,
    args: &[String],
    outfile: &Option<String>,
    append: bool,
    infile: &Option<String>,
) -> i32 {
    let mut process = Command::new(program);
    process.args(args);

    // Hook up any "< file", "> file", or ">> file" redirects
    redirect::apply_input(&mut process, infile);
    redirect::apply_output(&mut process, outfile, append);

    match process.spawn() {
        Ok(mut child) => match child.wait() {
            Ok(status) => status_code(status),
            Err(e) => {
                eprintln!("oxide: {}: failed to wait: {}", program, e);
                1
            }
        },
        Err(_) => {
            eprintln!("oxide: command not found: {}", program);
            127
        }
    }
}

/// Spawns a process in the background
pub fn spawn_background(
    program: &str,
    args: &[String],
    outfile: &Option<String>,
    append: bool,
    infile: &Option<String>,
) -> Result<Child, String> {
    let mut process = Command::new(program);
    process.args(args);

    redirect::apply_input(&mut process, infile);
    redirect::apply_output(&mut process, outfile, append);

    process
        .spawn()
        .map_err(|e| format!("oxide: failed to spawn {}: {}", program, e))
}
