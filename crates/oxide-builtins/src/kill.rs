use std::process::Command;

use crate::{Ctx, Io};

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    if args.is_empty() {
        ewln!(io, "oxide: kill: missing process ID (PID)");
        return 1;
    }

    let pid = &args[0];

    // Windows uses taskkill
    #[cfg(target_os = "windows")]
    let mut cmd = Command::new("taskkill");
    #[cfg(target_os = "windows")]
    cmd.args(["/F", "/PID", pid]); // /F means Force

    // Mac/Linux uses kill
    #[cfg(not(target_os = "windows"))]
    let mut cmd = Command::new("kill");
    #[cfg(not(target_os = "windows"))]
    cmd.arg("-9").arg(pid);

    // Capture rather than inherit, so the helper's output lands on our `io`
    // streams and stays inside a redirect or pipeline.
    match cmd.output() {
        Ok(output) => {
            let _ = io.stderr.write_all(&output.stderr);
            if output.status.success() {
                wln!(io, "oxide: process {} terminated.", pid);
                0
            } else {
                let _ = io.stdout.write_all(&output.stdout);
                1
            }
        }
        Err(e) => {
            ewln!(io, "oxide: kill: failed to execute: {}", e);
            1
        }
    }
}
