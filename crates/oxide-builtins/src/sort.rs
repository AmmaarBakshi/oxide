use std::cmp::Ordering;
use std::io::BufRead;

use crate::args::parse_flags;
use crate::{Ctx, Io};

struct Options {
    reverse: bool,
    numeric: bool,
    unique: bool,
    fold_case: bool,
}

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    let (flags, files) = match parse_flags(args, "rnuf") {
        Ok(parsed) => parsed,
        Err(msg) => {
            ewln!(io, "sort: {}", msg);
            ewln!(io, "Usage: sort [-rnuf] [file...]");
            return 2;
        }
    };
    let opts = Options {
        reverse: flags.contains('r'),
        numeric: flags.contains('n'),
        unique: flags.contains('u'),
        fold_case: flags.contains('f'),
    };

    let mut lines: Vec<Vec<u8>> = Vec::new();
    let status = io.each_input("sort", &files, |_, input, _| {
        for line in input.split(b'\n') {
            let mut line = line?;
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            lines.push(line);
        }
        Ok(())
    });
    // Sorting part of the input would be quietly wrong, so print nothing.
    if status != 0 {
        return 2;
    }

    if opts.unique {
        // Without the whole-line tiebreak, equal keys stay in input order and
        // `dedup` keeps the first of each run, as coreutils does.
        lines.sort_by(|a, b| directed(key_cmp(a, b, &opts), &opts));
        lines.dedup_by(|a, b| key_cmp(a, b, &opts) == Ordering::Equal);
    } else {
        // Equal keys fall back to comparing whole lines, so the output never
        // depends on the input order.
        lines.sort_by(|a, b| directed(key_cmp(a, b, &opts).then_with(|| a.cmp(b)), &opts));
    }

    for line in &lines {
        if io.stdout.write_all(line).is_err() || io.stdout.write_all(b"\n").is_err() {
            return 2;
        }
    }
    0
}

fn directed(ord: Ordering, opts: &Options) -> Ordering {
    if opts.reverse {
        ord.reverse()
    } else {
        ord
    }
}

/// Compares two lines by the key the options select.
fn key_cmp(a: &[u8], b: &[u8], opts: &Options) -> Ordering {
    if opts.numeric {
        leading_number(a).total_cmp(&leading_number(b))
    } else if opts.fold_case {
        let lower = |b: &u8| b.to_ascii_lowercase();
        a.iter().map(lower).cmp(b.iter().map(lower))
    } else {
        a.cmp(b)
    }
}

/// The number a line starts with, after leading blanks. As in coreutils, a
/// line that doesn't start with one sorts as zero.
fn leading_number(line: &[u8]) -> f64 {
    let s = line.trim_ascii_start();
    let mut end = usize::from(s.first() == Some(&b'-'));
    let mut seen_dot = false;
    while let Some(&b) = s.get(end) {
        if b == b'.' && !seen_dot {
            seen_dot = true;
        } else if !b.is_ascii_digit() {
            break;
        }
        end += 1;
    }
    std::str::from_utf8(&s[..end])
        .ok()
        .and_then(|n| n.parse().ok())
        .unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use crate::test_support::run;

    #[test]
    fn sorts_bytewise_by_default() {
        assert_eq!(run("sort", &[], "pear\nApple\napple\n").stdout, "Apple\napple\npear\n");
    }

    #[test]
    fn reverse() {
        assert_eq!(run("sort", &["-r"], "b\nc\na\n").stdout, "c\nb\na\n");
    }

    #[test]
    fn numeric_compares_values_not_text() {
        let out = run("sort", &["-n"], "10 ten\n9 nine\n-1.5\n100\nnone\n");
        assert_eq!(out.stdout, "-1.5\nnone\n9 nine\n10 ten\n100\n");
    }

    #[test]
    fn unique_keeps_first_of_each_equal_key() {
        assert_eq!(run("sort", &["-u"], "b\na\nb\na\n").stdout, "a\nb\n");
        assert_eq!(run("sort", &["-fu"], "B\na\nb\nA\n").stdout, "a\nB\n");
    }

    #[test]
    fn handles_crlf_and_a_missing_final_newline() {
        assert_eq!(run("sort", &[], "b\r\na").stdout, "a\nb\n");
    }

    #[test]
    fn unreadable_input_prints_nothing() {
        let out = run("sort", &["no-such-file.txt"], "");
        assert_eq!(out.status, 2);
        assert_eq!(out.stdout, "");
    }
}
