use crate::pipeline::SharedBuf;
use crate::redirect;
use oxide_parser::ast;
use std::io::Read;
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
    redirect::apply_outputs(process, cmd, None);
}

/// Runs a single command and returns its exit code. The redirects come from
/// `cmd`; `program` is its resolved path. Inside `$(...)`, stdout goes to
/// `capture`.
pub fn spawn_single(
    program: &str,
    args: &[String],
    cmd: &ast::Command,
    capture: Option<&SharedBuf>,
) -> i32 {
    let mut process = Command::new(program);
    process.args(args);

    let (reader, writer) = match capture.map(|_| std::io::pipe()) {
        None => (None, None),
        Some(Ok((reader, writer))) => (Some(reader), Some(writer)),
        Some(Err(e)) => {
            eprintln!("oxide: {}: {}", program, e);
            return 1;
        }
    };
    redirect::apply_input(&mut process, &cmd.infile);
    redirect::apply_outputs(&mut process, cmd, writer);

    let spawned = process.spawn();
    // Close our copy of the pipe's write end so the read below ends when
    // the child exits.
    drop(process);
    match spawned {
        Ok(mut child) => {
            if let (Some(mut reader), Some(capture)) = (reader, capture) {
                let mut bytes = Vec::new();
                let _ = reader.read_to_end(&mut bytes);
                capture.push(&bytes);
            }
            match child.wait() {
                Ok(status) => status_code(status),
                Err(e) => {
                    eprintln!("oxide: {}: failed to wait: {}", program, e);
                    1
                }
            }
        }
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
