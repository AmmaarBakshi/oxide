use std::fs::{read_to_string, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use crate::{Ctx, Io};

pub fn history_path() -> PathBuf {
    let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push(".oxide_history");
    path
}

pub fn load() -> Vec<String> {
    read_to_string(history_path())
        .unwrap_or_default()
        .lines()
        .map(|s| s.to_string())
        .collect()
}

pub fn append(line: &str) {
    if line.trim().is_empty() {
        return;
    }
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(history_path())
    {
        let _ = writeln!(file, "{}", line);
    }
}

pub fn run(_args: &[String], io: &mut Io<'_>, ctx: &mut Ctx<'_>) -> i32 {
    for (i, cmd) in ctx.history.iter().enumerate() {
        wln!(io, "{:>5}  {}", i + 1, cmd);
    }
    0
}
