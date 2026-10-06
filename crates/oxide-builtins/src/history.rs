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

pub fn run(args: &[String], io: &mut Io<'_>, ctx: &mut Ctx<'_>) -> i32 {
    // `history N` shows just the last N entries, keeping their numbers.
    let skip = match args.first() {
        None => 0,
        Some(n) => match n.parse::<usize>() {
            Ok(n) => ctx.history.len().saturating_sub(n),
            Err(_) => {
                ewln!(io, "oxide: history: invalid count '{}'", n);
                return 1;
            }
        },
    };

    for (i, cmd) in ctx.history.iter().enumerate().skip(skip) {
        wln!(io, "{:>5}  {}", i + 1, cmd);
    }
    0
}

#[cfg(test)]
mod tests {
    use crate::test_support::run_with_history;

    const HISTORY: [&str; 3] = ["ls", "cd src", "cat main.rs"];

    #[test]
    fn shows_everything_by_default() {
        let out = run_with_history("history", &[], &HISTORY);
        assert_eq!(out.stdout, "    1  ls\n    2  cd src\n    3  cat main.rs\n");
    }

    #[test]
    fn a_count_shows_the_last_entries() {
        let out = run_with_history("history", &["2"], &HISTORY);
        assert_eq!(out.stdout, "    2  cd src\n    3  cat main.rs\n");
        assert_eq!(run_with_history("history", &["10"], &HISTORY).stdout.lines().count(), 3);
    }

    #[test]
    fn rejects_a_bad_count() {
        let out = run_with_history("history", &["lots"], &HISTORY);
        assert_eq!(out.status, 1);
        assert!(out.stderr.contains("invalid count 'lots'"));
    }
}
