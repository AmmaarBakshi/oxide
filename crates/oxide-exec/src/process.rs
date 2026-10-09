use crate::redirect;
use oxide_parser::ast;
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

/// Hooks up all of `cmd`'s redirects on `process`.
fn apply_redirects(process: &mut Command, cmd: &ast::Command) {
    redirect::apply_input(process, &cmd.infile);
    redirect::apply_output(process, &cmd.outfile, cmd.append);
    redirect::apply_error(process, &cmd.errfile, cmd.err_append);
}

/// Runs a single command and returns its exit code. The redirects come from
/// `cmd`; `program` is its resolved path.
pub fn spawn_single(program: &str, args: &[String], cmd: &ast::Command) -> i32 {
    let mut process = Command::new(program);
    process.args(args);
    apply_redirects(&mut process, cmd);

    match process.spawn() {
        Ok(mut child) => match child.wait() {
            Ok(status) => status_code(status),
            Err(e) => {
                eprintln!("oxide: {}: failed to wait: {}", program, e);
                1
            }
        },
        Err(_) => {
            let message = format!("oxide: command not found: {}", program);
            redirect::report(&cmd.errfile, cmd.err_append, &message);
            127
        }
    }
}

/// Spawns a process in the background
pub fn spawn_background(
    program: &str,
    args: &[String],
    cmd: &ast::Command,
) -> Result<Child, String> {
    let mut process = Command::new(program);
    process.args(args);
    apply_redirects(&mut process, cmd);

    process
        .spawn()
        .map_err(|e| format!("oxide: failed to spawn {}: {}", program, e))
}
