use std::io::{self, BufRead, Write};

use crate::{Ctx, Io};

const DEFAULT_LINES: usize = 10;

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    let (count, files) = match parse_args(args) {
        Ok(parsed) => parsed,
        Err(msg) => {
            ewln!(io, "head: {}", msg);
            ewln!(io, "Usage: head [-n N] [file...]");
            return 1;
        }
    };

    // Like coreutils, label each file only when there is more than one.
    let show_names = files.len() > 1;
    let mut first = true;

    io.each_input("head", &files, |name, input, out| {
        if show_names {
            if !first {
                writeln!(out)?;
            }
            first = false;
            writeln!(out, "==> {} <==", display_name(name))?;
        }
        copy_lines(input, out, count)
    })
}

/// Splits `args` into the line count and the file operands. Accepts `-n N`,
/// `-nN`, and the traditional `-N`.
fn parse_args(args: &[String]) -> Result<(usize, Vec<&str>), String> {
    let mut count = DEFAULT_LINES;
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
        count = value
            .parse()
            .map_err(|_| format!("invalid number of lines: '{}'", value))?;
    }
    Ok((count, files))
}

/// Copies up to `n` lines from `input` to `out`, then stops reading rather
/// than draining whatever is left upstream.
fn copy_lines(input: &mut dyn BufRead, out: &mut dyn Write, n: usize) -> io::Result<()> {
    let mut line = Vec::new();
    for _ in 0..n {
        line.clear();
        if input.read_until(b'\n', &mut line)? == 0 {
            break;
        }
        out.write_all(&line)?;
    }
    Ok(())
}

fn display_name(name: &str) -> &str {
    if name == "-" {
        "standard input"
    } else {
        name
    }
}

#[cfg(test)]
mod tests {
    use crate::test_support::run;

    const TWELVE: &str = "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n";

    #[test]
    fn defaults_to_ten_lines() {
        assert_eq!(run("head", &[], TWELVE).stdout, "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n");
    }

    #[test]
    fn accepts_every_count_spelling() {
        for args in [&["-n", "2"][..], &["-n2"], &["-2"]] {
            assert_eq!(run("head", args, TWELVE).stdout, "1\n2\n", "args: {:?}", args);
        }
    }

    #[test]
    fn short_input_is_passed_through_whole() {
        assert_eq!(run("head", &["-n", "5"], "a\nb").stdout, "a\nb");
    }

    #[test]
    fn rejects_a_bad_count() {
        let out = run("head", &["-n", "lots"], TWELVE);
        assert_eq!(out.status, 1);
        assert!(out.stderr.contains("invalid number of lines"));
    }

    #[test]
    fn missing_file_is_reported() {
        let out = run("head", &["no-such-file.txt"], "");
        assert_eq!(out.status, 1);
        assert!(out.stderr.starts_with("head: no-such-file.txt:"));
    }
}
