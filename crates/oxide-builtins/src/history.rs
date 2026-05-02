use std::fs::{OpenOptions, read_to_string};
use std::io::Write;
use std::path::PathBuf;

fn get_history_path() -> PathBuf {
    let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push(".oxide_history");
    path
}

pub fn load() -> Vec<String> {
    read_to_string(get_history_path())
        .unwrap_or_default()
        .lines()
        .map(|s| s.to_string())
        .collect()
}

pub fn append(line: &str) {
    if line.trim().is_empty() { return; }
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(get_history_path()) 
    {
        let _ = writeln!(file, "{}", line);
    }
}

pub fn execute(history: &[String]) -> i32 {
    for (i, cmd) in history.iter().enumerate() {
        println!("{:>5}  {}", i + 1, cmd);
    }
    0
}