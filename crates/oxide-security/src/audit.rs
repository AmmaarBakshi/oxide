use std::fs::OpenOptions;
use std::io::Write;
use std::time::SystemTime;

pub fn log_command(cmd: &str, args: &[String], allowed: bool) {
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open("oxide_audit.log") {
        let status = if allowed { "ALLOWED" } else { "BLOCKED" };
        let timestamp = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs();
        let _ = writeln!(file, "[{}] {} | {} {:?}", timestamp, status, cmd, args);
    }
}