use std::io::{self, BufRead, Write};

use crate::args::parse_flags;
use crate::{Ctx, Io};

struct Options {
    count: bool,
    repeated_only: bool,
    unique_only: bool,
    ignore_case: bool,
}

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    let (flags, files) = match parse_flags(args, "cdui") {
        Ok(parsed) => parsed,
        Err(msg) => {
            ewln!(io, "uniq: {}", msg);
            ewln!(io, "Usage: uniq [-cdui] [file]");
            return 1;
        }
    };
    if let Some(extra) = files.get(1) {
        ewln!(io, "uniq: extra operand '{}'", extra);
        return 1;
    }
    let opts = Options {
        count: flags.contains('c'),
        repeated_only: flags.contains('d'),
        unique_only: flags.contains('u'),
        ignore_case: flags.contains('i'),
    };

    io.each_input("uniq", &files, |_, input, out| collapse(input, out, &opts))
}

/// Streams `input` to `out`, folding each run of adjacent equal lines into
/// one. Only the current run is held in memory.
fn collapse(input: &mut dyn BufRead, out: &mut dyn Write, opts: &Options) -> io::Result<()> {
    let mut current: Option<(Vec<u8>, u64)> = None;
    let mut line = Vec::new();

    loop {
        line.clear();
        if input.read_until(b'\n', &mut line)? == 0 {
            break;
        }
        trim_newline(&mut line);

        match &mut current {
            Some((text, n)) if same(text, &line, opts.ignore_case) => *n += 1,
            _ => {
                if let Some((text, n)) = current.take() {
                    emit(out, &text, n, opts)?;
                }
                current = Some((std::mem::take(&mut line), 1));
            }
        }
    }

    match current {
        Some((text, n)) => emit(out, &text, n, opts),
        None => Ok(()),
    }
}

fn same(a: &[u8], b: &[u8], ignore_case: bool) -> bool {
    if ignore_case {
        a.eq_ignore_ascii_case(b)
    } else {
        a == b
    }
}

/// Writes one collapsed run, unless `-d` or `-u` filters it out.
fn emit(out: &mut dyn Write, text: &[u8], n: u64, opts: &Options) -> io::Result<()> {
    if (opts.repeated_only && n < 2) || (opts.unique_only && n > 1) {
        return Ok(());
    }
    if opts.count {
        write!(out, "{:>7} ", n)?;
    }
    out.write_all(text)?;
    out.write_all(b"\n")
}

fn trim_newline(line: &mut Vec<u8>) {
    if line.last() == Some(&b'\n') {
        line.pop();
        if line.last() == Some(&b'\r') {
            line.pop();
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::test_support::run;

    const INPUT: &str = "a\na\nb\nc\nc\nc\na\n";

    #[test]
    fn collapses_only_adjacent_duplicates() {
        assert_eq!(run("uniq", &[], INPUT).stdout, "a\nb\nc\na\n");
    }

    #[test]
    fn counts_each_run() {
        let out = run("uniq", &["-c"], INPUT);
        assert_eq!(out.stdout, "      2 a\n      1 b\n      3 c\n      1 a\n");
    }

    #[test]
    fn filters_repeated_or_unique_runs() {
        assert_eq!(run("uniq", &["-d"], INPUT).stdout, "a\nc\n");
        assert_eq!(run("uniq", &["-u"], INPUT).stdout, "b\na\n");
    }

    #[test]
    fn ignore_case_keeps_the_first_spelling() {
        assert_eq!(run("uniq", &["-ic"], "Hi\nhI\nhi\n").stdout, "      3 Hi\n");
    }

    #[test]
    fn treats_crlf_and_lf_lines_as_equal() {
        assert_eq!(run("uniq", &[], "x\r\nx\nx").stdout, "x\n");
    }

    #[test]
    fn rejects_more_than_one_input() {
        let out = run("uniq", &["a.txt", "b.txt"], "");
        assert_eq!(out.status, 1);
        assert!(out.stderr.contains("extra operand 'b.txt'"));
    }
}
