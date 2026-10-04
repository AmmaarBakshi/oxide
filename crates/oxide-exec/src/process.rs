use std::process::{Child, Command};
use crate::redirect;

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

    process.spawn().map_err(|e| format!("oxide: failed to spawn {}: {}", program, e))
}
