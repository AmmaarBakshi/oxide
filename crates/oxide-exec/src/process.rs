use std::process::{Command, Stdio, Child};
use crate::redirect;

/// Runs a single command and returns its exit code
pub fn spawn_single(program: &str, args: &[String], outfile: &Option<String>) -> i32 {
    let mut process = Command::new(program);
    process.args(args);

    // Hook up any "> file.txt" redirects
    redirect::apply(&mut process, outfile);

    match process.spawn() {
        Ok(mut child) => {
            let status = child.wait().expect("failed to wait");
            status.code().unwrap_or(1)
        }
        Err(_) => {
            eprintln!("oxide: command not found: {}", program);
            127
        }
    }
}

/// Spawns a command as part of a pipeline (handles chaining inputs/outputs)
pub fn spawn_piped(
    program: &str,
    args: &[String],
    stdin: Option<std::process::ChildStdout>,
    is_last: bool,
    outfile: &Option<String>,
) -> Result<Child, String> {
    let mut process = Command::new(program);
    process.args(args);

    // If there is output from the previous command, pipe it into this one!
    if let Some(stdout) = stdin {
        process.stdin(Stdio::from(stdout));
    }

    if !is_last {
        // If it's NOT the last command, pipe its output forward
        process.stdout(Stdio::piped());
    } else {
        // If it IS the last command, handle any "> file.txt" redirects
        redirect::apply(&mut process, outfile);
    }

    process.spawn().map_err(|_| format!("oxide: command not found: {}", program))
}

/// Spawns a process in the background (does NOT wait for it to finish)
pub fn spawn_background(
    program: &str, 
    args: &[String], 
    outfile: &Option<String>
) -> Result<Child, String> {
    let mut process = Command::new(program);
    process.args(args);

    redirect::apply(&mut process, outfile);

    process.spawn().map_err(|e| format!("oxide: failed to spawn {}: {}", program, e))
}