use crate::{Ctx, Io};

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    if args.is_empty() {
        ewln!(io, "grep: usage: grep <pattern> [file...]");
        return 2;
    }

    let pattern = &args[0];
    let files = &args[1..];

    // No file operands: filter stdin instead. This is what makes grep usable
    // as a pipeline stage (`ls | grep .rs`).
    if files.is_empty() {
        return grep_stdin(pattern, io);
    }

    let mut status = 1; // grep exits 1 when nothing matched
    let show_names = files.len() > 1;

    for path in files {
        match std::fs::read_to_string(path) {
            Ok(content) => {
                for (i, line) in content.lines().enumerate() {
                    if line.contains(pattern) {
                        status = 0;
                        if show_names {
                            wln!(io, "{}:{}: {}", path, i + 1, line);
                        } else {
                            wln!(io, "{}", line);
                        }
                    }
                }
            }
            Err(e) => {
                ewln!(io, "grep: {}: {}", path, e);
                return 2;
            }
        }
    }
    status
}

fn grep_stdin(pattern: &str, io: &mut Io<'_>) -> i32 {
    let mut status = 1;
    let mut line = String::new();
    loop {
        line.clear();
        match io.stdin.read_line(&mut line) {
            Ok(0) => return status,
            Ok(_) => {
                let trimmed = line.trim_end_matches(['\n', '\r']);
                if trimmed.contains(pattern) {
                    status = 0;
                    wln!(io, "{}", trimmed);
                }
            }
            Err(e) => {
                ewln!(io, "grep: {}", e);
                return 2;
            }
        }
    }
}
