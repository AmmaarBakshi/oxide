use std::process::Command;

use crate::{Ctx, Io};

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    if args.is_empty() {
        ewln!(io, "oxide: kill: missing process ID (PID)");
        return 1;
    }

    // Only plain numbers. Each PID is handed to taskkill/kill, so anything
    // else could be read there as an option (`kill /IM explorer.exe`).
    if let Some(bad) = args.iter().find(|pid| pid.parse::<u32>().is_err()) {
        ewln!(io, "oxide: kill: invalid process ID '{}'", bad);
        return 1;
    }

    let mut status = 0;
    for pid in args {
        status |= kill_one(pid, io);
    }
    status
}

fn kill_one(pid: &str, io: &mut Io<'_>) -> i32 {
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

#[cfg(test)]
mod tests {
    use crate::test_support::run;

    #[test]
    fn rejects_anything_but_numeric_pids() {
        for args in [&["abc"][..], &["/IM", "explorer.exe"], &["-9"], &["123", "x"]] {
            let out = run("kill", args, "");
            assert_eq!(out.status, 1, "args: {:?}", args);
            assert!(out.stderr.contains("invalid process ID"), "args: {:?}", args);
        }
    }
}
