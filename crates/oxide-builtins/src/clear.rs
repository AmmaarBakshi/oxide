use std::io::{self, Write};

pub fn execute(_args: &[String]) -> i32 {
    let mut stdout = io::stdout();

    // Clear screen + clear scrollback/history + move cursor to top-left
    if write!(stdout, "\x1B[3J\x1B[2J\x1B[H").is_err() {
        return 1;
    }

    if stdout.flush().is_err() {
        return 1;
    }

    0
}