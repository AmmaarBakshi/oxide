use std::io::{self, BufRead, Write};

use crate::{Ctx, Io};

struct Options {
    /// Number blank lines too (`-b a`), not just ones with text (`-b t`).
    all: bool,
    width: usize,
    separator: String,
}

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    let (opts, files) = match parse_args(args) {
        Ok(parsed) => parsed,
        Err(msg) => {
            ewln!(io, "nl: {}", msg);
            ewln!(io, "Usage: nl [-b a|t] [-w WIDTH] [-s SEP] [file...]");
            return 1;
        }
    };

    // Numbering runs on across files, as if they were one input.
    let mut n = 0u64;
    io.each_input("nl", &files, |_, input, out| {
        number(input, out, &opts, &mut n)
    })
}

fn parse_args(args: &[String]) -> Result<(Options, Vec<&str>), String> {
    let mut opts = Options {
        all: false,
        width: 6,
        separator: "\t".to_string(),
    };
    let mut files = Vec::new();

    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == "--" {
            files.extend(iter.by_ref().map(String::as_str));
            break;
        }
        let Some(flag) = arg.strip_prefix('-').and_then(|rest| rest.chars().next()) else {
            files.push(arg.as_str());
            continue;
        };
        if !"bws".contains(flag) {
            return Err(format!("invalid option -- '{}'", flag));
        }
        let value = match &arg[2..] {
            "" => iter
                .next()
                .map(String::as_str)
                .ok_or_else(|| format!("option requires an argument -- '{}'", flag))?,
            attached => attached,
        };
        match flag {
            'b' => {
                opts.all = match value {
                    "a" => true,
                    "t" => false,
                    _ => return Err(format!("invalid line numbering style: '{}'", value)),
                }
            }
            'w' => {
                opts.width = match value.parse() {
                    Ok(width) if width > 0 => width,
                    _ => return Err(format!("invalid line number field width: '{}'", value)),
                }
            }
            _ => opts.separator = value.to_string(),
        }
    }
    Ok((opts, files))
}

fn number(
    input: &mut dyn BufRead,
    out: &mut dyn Write,
    opts: &Options,
    n: &mut u64,
) -> io::Result<()> {
    let mut line = Vec::new();
    loop {
        line.clear();
        if input.read_until(b'\n', &mut line)? == 0 {
            return Ok(());
        }
        let blank = matches!(&line[..], b"\n" | b"\r\n");
        if blank && !opts.all {
            // An unnumbered line is indented to match the numbered ones.
            let indent = opts.width + opts.separator.chars().count();
            write!(out, "{:indent$}", "")?;
        } else {
            *n += 1;
            write!(out, "{:>width$}{}", n, opts.separator, width = opts.width)?;
        }
        out.write_all(&line)?;
    }
}

#[cfg(test)]
mod tests {
    use crate::test_support::run;

    #[test]
    fn numbers_lines_with_text() {
        let out = run("nl", &[], "a\n\nb\n");
        assert_eq!(out.stdout, "     1\ta\n       \n     2\tb\n");
    }

    #[test]
    fn dash_b_a_numbers_blank_lines_too() {
        let out = run("nl", &["-ba"], "a\n\nb\n");
        assert_eq!(out.stdout, "     1\ta\n     2\t\n     3\tb\n");
    }

    #[test]
    fn width_and_separator() {
        let out = run("nl", &["-w", "2", "-s", ": "], "x\ny\n");
        assert_eq!(out.stdout, " 1: x\n 2: y\n");
    }

    #[test]
    fn rejects_bad_values() {
        let out = run("nl", &["-b", "z"], "");
        assert_eq!(out.status, 1);
        assert!(out.stderr.contains("invalid line numbering style"));
        let out = run("nl", &["-w", "0"], "");
        assert_eq!(out.status, 1);
        assert!(out.stderr.contains("invalid line number field width"));
    }
}
