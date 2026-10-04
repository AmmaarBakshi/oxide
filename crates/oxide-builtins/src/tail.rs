use std::collections::VecDeque;
use std::io::{self, BufRead, Write};

use crate::{Ctx, Io};

const DEFAULT_LINES: usize = 10;

/// Which part of the input to print.
#[derive(Clone, Copy)]
enum Start {
    /// The last N lines.
    Last(usize),
    /// Everything from line N (1-based) onward, written `-n +N`.
    From(usize),
}

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    let (start, files) = match parse_args(args) {
        Ok(parsed) => parsed,
        Err(msg) => {
            ewln!(io, "tail: {}", msg);
            ewln!(io, "Usage: tail [-n N | -n +N] [file...]");
            return 1;
        }
    };

    // Like coreutils, label each file only when there is more than one.
    let show_names = files.len() > 1;
    let mut first = true;

    io.each_input("tail", &files, |name, input, out| {
        if show_names {
            if !first {
                writeln!(out)?;
            }
            first = false;
            let name = if name == "-" { "standard input" } else { name };
            writeln!(out, "==> {} <==", name)?;
        }
        match start {
            Start::Last(n) => copy_last(input, out, n),
            Start::From(n) => copy_from(input, out, n),
        }
    })
}

/// Splits `args` into the start position and the file operands. Accepts
/// `-n N`, `-nN`, `-N`, and a `+` prefix on the count to mean "from line N".
fn parse_args(args: &[String]) -> Result<(Start, Vec<&str>), String> {
    let mut start = Start::Last(DEFAULT_LINES);
    let mut files = Vec::new();

    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let value = if arg == "-n" {
            iter.next().ok_or("option requires an argument -- 'n'")?
        } else if let Some(value) = arg.strip_prefix("-n") {
            value
        } else if arg.len() > 1 && arg.starts_with('-') {
            let value = &arg[1..];
            if !value.bytes().all(|b| b.is_ascii_digit()) {
                return Err(format!("invalid option '{}'", arg));
            }
            value
        } else {
            // Includes a lone `-`, which `each_input` reads as stdin.
            files.push(arg.as_str());
            continue;
        };

        let invalid = || format!("invalid number of lines: '{}'", value);
        start = match value.strip_prefix('+') {
            Some(from) => Start::From(from.parse().map_err(|_| invalid())?),
            None => Start::Last(value.parse().map_err(|_| invalid())?),
        };
    }
    Ok((start, files))
}

/// Writes the last `n` lines of `input`. The input has to be read to the end
/// to find them, but only `n` lines are ever held in memory.
fn copy_last(input: &mut dyn BufRead, out: &mut dyn Write, n: usize) -> io::Result<()> {
    if n == 0 {
        return Ok(());
    }

    let mut window: VecDeque<Vec<u8>> = VecDeque::with_capacity(n);
    let mut line = Vec::new();
    while input.read_until(b'\n', &mut line)? > 0 {
        let spare = if window.len() == n {
            // Recycle the evicted line's buffer instead of allocating a new one.
            window.pop_front().unwrap_or_default()
        } else {
            Vec::new()
        };
        window.push_back(std::mem::replace(&mut line, spare));
        line.clear();
    }

    for line in &window {
        out.write_all(line)?;
    }
    Ok(())
}

/// Skips the first `n - 1` lines of `input` and streams the rest through.
fn copy_from(input: &mut dyn BufRead, out: &mut dyn Write, n: usize) -> io::Result<()> {
    let mut skipped = Vec::new();
    for _ in 1..n {
        skipped.clear();
        if input.read_until(b'\n', &mut skipped)? == 0 {
            return Ok(());
        }
    }
    io::copy(input, out).map(drop)
}

#[cfg(test)]
mod tests {
    use crate::test_support::run;

    const TWELVE: &str = "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n";

    #[test]
    fn defaults_to_last_ten_lines() {
        assert_eq!(run("tail", &[], TWELVE).stdout, "3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n");
    }

    #[test]
    fn accepts_every_count_spelling() {
        for args in [&["-n", "2"][..], &["-n2"], &["-2"]] {
            assert_eq!(run("tail", args, TWELVE).stdout, "11\n12\n", "args: {:?}", args);
        }
    }

    #[test]
    fn plus_count_starts_from_that_line() {
        assert_eq!(run("tail", &["-n", "+11"], TWELVE).stdout, "11\n12\n");
        assert_eq!(run("tail", &["-n", "+1"], "a\nb\n").stdout, "a\nb\n");
        assert_eq!(run("tail", &["-n", "+5"], "a\nb\n").stdout, "");
    }

    #[test]
    fn zero_lines_prints_nothing() {
        assert_eq!(run("tail", &["-n", "0"], TWELVE).stdout, "");
    }

    #[test]
    fn keeps_an_unterminated_last_line() {
        assert_eq!(run("tail", &["-n", "1"], "a\nb").stdout, "b");
    }

    #[test]
    fn rejects_a_bad_count() {
        let out = run("tail", &["-n", "+x"], TWELVE);
        assert_eq!(out.status, 1);
        assert!(out.stderr.contains("invalid number of lines: '+x'"));
    }
}
