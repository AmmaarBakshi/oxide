use std::process::Command;

pub fn execute(args: &[String]) -> i32 {
    if args.is_empty() {
        eprintln!("oxide: kill: missing process ID (PID)");
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

    match cmd.status() {
        Ok(status) => {
            if status.success() {
                println!("oxide: process {} terminated.", pid);
                0
            } else {
                1
            }
        }
        Err(e) => {
            eprintln!("oxide: kill: failed to execute: {}", e);
            1
        }
    }
}