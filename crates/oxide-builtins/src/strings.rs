use std::io::{self, BufRead, Write};

use crate::{Ctx, Io};

const DEFAULT_MIN: usize = 4;

/// The base `-t` prints each string's offset in.
#[derive(Clone, Copy)]
enum Radix {
    Decimal,
    Octal,
    Hex,
}

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    let (min, radix, files) = match parse_args(args) {
        Ok(parsed) => parsed,
        Err(msg) => {
            ewln!(io, "strings: {}", msg);
            ewln!(io, "Usage: strings [-n MIN] [-t d|o|x] [file...]");
            return 1;
        }
    };
    io.each_input("strings", &files, |_, input, out| {
        strings(input, out, min, radix)
    })
}

/// Accepts `-n N`, `-nN` and the traditional `-N` for the minimum length,
/// and `-t d|o|x` (or `-td` and so on) for offsets.
fn parse_args(args: &[String]) -> Result<(usize, Option<Radix>, Vec<&str>), String> {
    let mut min = DEFAULT_MIN;
    let mut radix = None;
    let mut files = Vec::new();

    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let arg = arg.as_str();
        if arg == "--" {
            files.extend(iter.by_ref().map(String::as_str));
            break;
        }
        if let Some(value) = arg.strip_prefix("-t") {
            let value = match value {
                "" => iter.next().ok_or("option requires an argument -- 't'")?,
                value => value,
            };
            radix = Some(match value {
                "d" => Radix::Decimal,
                "o" => Radix::Octal,
                "x" => Radix::Hex,
                _ => return Err(format!("invalid radix: '{}'", value)),
            });
            continue;
        }
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
            files.push(arg);
            continue;
        };
        min = match value.parse() {
            Ok(min) if min > 0 => min,
            _ => return Err(format!("invalid minimum string length: '{}'", value)),
        };
    }
    Ok((min, radix, files))
}

/// Prints every run of at least `min` printable ASCII characters (tabs
/// included) in `input`, one per line. The input is read a buffer at a
/// time, so a large binary needn't fit in memory.
fn strings(
    input: &mut dyn BufRead,
    out: &mut dyn Write,
    min: usize,
    radix: Option<Radix>,
) -> io::Result<()> {
    let mut run = Vec::new();
    let (mut offset, mut start) = (0u64, 0u64);
    loop {
        let buf = input.fill_buf()?;
        if buf.is_empty() {
            break;
        }
        for &byte in buf {
            if byte == b'\t' || (b' '..=b'~').contains(&byte) {
                if run.is_empty() {
                    start = offset;
                }
                run.push(byte);
            } else {
                emit(out, &mut run, start, min, radix)?;
            }
            offset += 1;
        }
        let read = buf.len();
        input.consume(read);
    }
    emit(out, &mut run, start, min, radix)
}

/// Writes the pending run if it is long enough, then starts a new one.
fn emit(
    out: &mut dyn Write,
    run: &mut Vec<u8>,
    start: u64,
    min: usize,
    radix: Option<Radix>,
) -> io::Result<()> {
    if run.len() >= min {
        match radix {
            Some(Radix::Decimal) => write!(out, "{:>7} ", start)?,
            Some(Radix::Octal) => write!(out, "{:>7o} ", start)?,
            Some(Radix::Hex) => write!(out, "{:>7x} ", start)?,
            None => {}
        }
        out.write_all(run)?;
        out.write_all(b"\n")?;
    }
    run.clear();
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::test_support::run;

    #[test]
    fn prints_runs_of_at_least_four_printable_characters() {
        let out = run("strings", &[], "ab\0hello\x01\x02world!\0xyz");
        assert_eq!(out.status, 0, "{}", out.stderr);
        assert_eq!(out.stdout, "hello\nworld!\n");
    }

    #[test]
    fn non_ascii_bytes_break_a_run() {
        assert_eq!(run("strings", &[], "caféteria").stdout, "teria\n");
    }

    #[test]
    fn the_minimum_length_can_be_changed() {
        let input = "ab\0abc\0abcd";
        assert_eq!(
            run("strings", &["-n", "2"], input).stdout,
            "ab\nabc\nabcd\n"
        );
        assert_eq!(run("strings", &["-3"], input).stdout, "abc\nabcd\n");
        assert_eq!(run("strings", &["-n5"], input).stdout, "");
    }

    #[test]
    fn dash_t_prefixes_each_offset() {
        let input = "\0\0hello\0\0\0\0\0\0\0\0world";
        assert_eq!(
            run("strings", &["-t", "x"], input).stdout,
            "      2 hello\n      f world\n"
        );
        assert_eq!(
            run("strings", &["-td"], input).stdout,
            "      2 hello\n     15 world\n"
        );
        assert_eq!(
            run("strings", &["-to"], input).stdout,
            "      2 hello\n     17 world\n"
        );
    }

    #[test]
    fn rejects_bad_options() {
        let out = run("strings", &["-n", "0"], "");
        assert_eq!(out.status, 1);
        assert!(out.stderr.contains("invalid minimum string length"));

        let out = run("strings", &["-t", "b"], "");
        assert!(out.stderr.contains("invalid radix: 'b'"));
    }
}
