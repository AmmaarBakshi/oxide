use crate::args::parse_flags;
use crate::{Ctx, Io};

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    let (flags, operands) = match parse_flags(args, "ivn") {
        Ok(parsed) => parsed,
        Err(msg) => {
            ewln!(io, "grep: {}", msg);
            ewln!(io, "Usage: grep [-ivn] <pattern> [file...]");
            return 2;
        }
    };
    let Some((pattern, files)) = operands.split_first() else {
        ewln!(io, "grep: usage: grep [-ivn] <pattern> [file...]");
        return 2;
    };

    let ignore_case = flags.contains('i');
    let invert = flags.contains('v');
    let line_numbers = flags.contains('n');

    // Lowercased once here rather than on every line.
    let pattern = if ignore_case { pattern.to_lowercase() } else { pattern.to_string() };
    let show_names = files.len() > 1;
    let mut matched = false;

    // No file operands: filter stdin instead. This is what makes grep usable
    // as a pipeline stage (`ls | grep .rs`).
    let status = io.each_input("grep", files, |name, input, out| {
        let mut buf = Vec::new();
        let mut number = 0;
        loop {
            buf.clear();
            if input.read_until(b'\n', &mut buf)? == 0 {
                return Ok(());
            }
            number += 1;

            // Lossy, so one stray non-UTF-8 byte doesn't abort the whole file.
            let line = String::from_utf8_lossy(&buf);
            let line = line.trim_end_matches(['\n', '\r']);
            let hit = if ignore_case {
                line.to_lowercase().contains(&pattern)
            } else {
                line.contains(&pattern)
            };
            if hit == invert {
                continue;
            }

            matched = true;
            if show_names {
                write!(out, "{}:", name)?;
            }
            if line_numbers {
                write!(out, "{}:", number)?;
            }
            writeln!(out, "{}", line)?;
        }
    });

    // grep's convention: 2 if any input failed, else 0 for a match, 1 for none.
    if status != 0 {
        2
    } else if matched {
        0
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use crate::test_support::run;

    const INPUT: &str = "Apple pie\nbanana\napple tart\n";

    #[test]
    fn prints_matching_lines() {
        let out = run("grep", &["apple"], INPUT);
        assert_eq!((out.status, out.stdout.as_str()), (0, "apple tart\n"));
    }

    #[test]
    fn flags_ignore_case_invert_and_number() {
        assert_eq!(run("grep", &["-i", "APPLE"], INPUT).stdout, "Apple pie\napple tart\n");
        assert_eq!(run("grep", &["-v", "apple"], INPUT).stdout, "Apple pie\nbanana\n");
        assert_eq!(run("grep", &["-in", "apple"], INPUT).stdout, "1:Apple pie\n3:apple tart\n");
    }

    #[test]
    fn exit_status_follows_grep_convention() {
        assert_eq!(run("grep", &["cherry"], INPUT).status, 1);
        assert_eq!(run("grep", &["x", "no-such-file.txt"], "").status, 2);
        assert_eq!(run("grep", &[], "").status, 2);
    }
}
