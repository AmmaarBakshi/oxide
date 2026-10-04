use std::fs;

use crate::{Ctx, Io};

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    // No operands means "copy stdin through", which is what makes
    // `cat < in.txt | grep foo` and a bare `cat` in a pipeline work.
    if args.is_empty() {
        return copy_stdin(io);
    }

    for path in args {
        match fs::read_to_string(path) {
            Ok(content) => {
                if io.stdout.write_all(content.as_bytes()).is_err() {
                    return 1;
                }
            }
            Err(e) => {
                ewln!(io, "cat: {}: {}", path, e);
                return 1;
            }
        }
    }
    0
}

/// Streams stdin to stdout a line at a time, so a long-running upstream
/// stage shows up as it arrives rather than only at EOF.
fn copy_stdin(io: &mut Io<'_>) -> i32 {
    let mut line = String::new();
    loop {
        line.clear();
        match io.stdin.read_line(&mut line) {
            Ok(0) => return 0,
            Ok(_) => {
                if io.stdout.write_all(line.as_bytes()).is_err() {
                    return 1;
                }
            }
            Err(e) => {
                ewln!(io, "cat: {}", e);
                return 1;
            }
        }
    }
}
