use std::io::{self, BufRead, Write};

use crate::{Ctx, Io};

/// Whether `-f` is splitting on a delimiter or `-c` is picking characters.
enum Mode {
    Fields {
        delimiter: char,
        only_delimited: bool,
    },
    Chars,
}

struct Options {
    mode: Mode,
    /// Inclusive, 1-based ranges, sorted and merged. `usize::MAX` is an
    /// open end (`3-`).
    ranges: Vec<(usize, usize)>,
}

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    let (opts, files) = match parse_args(args) {
        Ok(parsed) => parsed,
        Err(msg) => {
            ewln!(io, "cut: {}", msg);
            ewln!(
                io,
                "Usage: cut -f LIST [-d DELIM] [-s] [file...] | cut -c LIST [file...]"
            );
            return 1;
        }
    };
    io.each_input("cut", &files, |_, input, out| cut(input, out, &opts))
}

fn parse_args(args: &[String]) -> Result<(Options, Vec<&str>), String> {
    let mut fields = None;
    let mut chars = None;
    let mut delimiter = '\t';
    let mut only_delimited = false;
    let mut files = Vec::new();

    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == "--" {
            files.extend(iter.by_ref().map(String::as_str));
            break;
        }
        if arg == "-s" {
            only_delimited = true;
            continue;
        }
        // `-f`, `-c` and `-d` take a value, attached (`-f2`) or not (`-f 2`).
        let Some(flag) = arg.strip_prefix('-').and_then(|rest| rest.chars().next()) else {
            files.push(arg.as_str());
            continue;
        };
        if !"fcd".contains(flag) {
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
            'f' => fields = Some(parse_list(value)?),
            'c' => chars = Some(parse_list(value)?),
            _ => {
                let mut it = value.chars();
                delimiter = match (it.next(), it.next()) {
                    (Some(c), None) => c,
                    _ => return Err("the delimiter must be a single character".into()),
                };
            }
        }
    }

    let (mode, ranges) = match (fields, chars) {
        (Some(_), Some(_)) => return Err("only one list may be specified".into()),
        (Some(ranges), None) => (
            Mode::Fields {
                delimiter,
                only_delimited,
            },
            ranges,
        ),
        (None, Some(ranges)) => (Mode::Chars, ranges),
        (None, None) => return Err("you must specify a list of fields or characters".into()),
    };
    Ok((Options { mode, ranges }, files))
}

/// Parses a list like `1,3-5,7-` into sorted, merged ranges.
fn parse_list(list: &str) -> Result<Vec<(usize, usize)>, String> {
    let position = |s: &str| -> Result<usize, String> {
        match s.parse::<usize>() {
            Ok(0) => Err("fields and positions are numbered from 1".into()),
            Ok(n) => Ok(n),
            Err(_) => Err(format!("invalid field value '{}'", s)),
        }
    };

    let mut ranges = Vec::new();
    for part in list.split(',') {
        let range = match part.split_once('-') {
            None => {
                let n = position(part)?;
                (n, n)
            }
            Some(("", "")) => return Err("invalid range with no endpoint: -".into()),
            Some(("", end)) => (1, position(end)?),
            Some((start, "")) => (position(start)?, usize::MAX),
            Some((start, end)) => {
                let (start, end) = (position(start)?, position(end)?);
                if end < start {
                    return Err("invalid decreasing range".into());
                }
                (start, end)
            }
        };
        ranges.push(range);
    }

    ranges.sort_unstable();
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for (start, end) in ranges {
        match merged.last_mut() {
            Some(last) if start <= last.1.saturating_add(1) => last.1 = last.1.max(end),
            _ => merged.push((start, end)),
        }
    }
    Ok(merged)
}

fn selected(ranges: &[(usize, usize)], n: usize) -> bool {
    ranges.iter().any(|&(start, end)| start <= n && n <= end)
}

fn cut(input: &mut dyn BufRead, out: &mut dyn Write, opts: &Options) -> io::Result<()> {
    let mut line = String::new();
    loop {
        line.clear();
        if input.read_line(&mut line)? == 0 {
            return Ok(());
        }
        let text = line.trim_end_matches('\n').trim_end_matches('\r');

        match opts.mode {
            Mode::Chars => {
                let picked: String = text
                    .chars()
                    .enumerate()
                    .filter(|&(i, _)| selected(&opts.ranges, i + 1))
                    .map(|(_, c)| c)
                    .collect();
                writeln!(out, "{}", picked)?;
            }
            // A line with no delimiter at all is passed through whole,
            // unless `-s` asks for such lines to be dropped.
            Mode::Fields {
                delimiter,
                only_delimited,
            } if !text.contains(delimiter) => {
                if !only_delimited {
                    writeln!(out, "{}", text)?;
                }
            }
            Mode::Fields { delimiter, .. } => {
                let picked: Vec<&str> = text
                    .split(delimiter)
                    .enumerate()
                    .filter(|&(i, _)| selected(&opts.ranges, i + 1))
                    .map(|(_, field)| field)
                    .collect();
                writeln!(out, "{}", picked.join(&delimiter.to_string()))?;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::test_support::run;

    #[test]
    fn picks_fields_in_input_order() {
        let out = run("cut", &["-d", ":", "-f", "3,1"], "a:b:c:d\n");
        assert_eq!(out.stdout, "a:c\n");
        let out = run("cut", &["-d,", "-f2-"], "x,y,z\n");
        assert_eq!(out.stdout, "y,z\n");
    }

    #[test]
    fn splits_on_tabs_by_default() {
        assert_eq!(run("cut", &["-f2"], "one\ttwo\tthree\n").stdout, "two\n");
    }

    #[test]
    fn lines_without_the_delimiter_pass_through_unless_dash_s() {
        let out = run("cut", &["-d", " ", "-f2"], "abc\na b\n");
        assert_eq!(out.stdout, "abc\nb\n");
        let out = run("cut", &["-s", "-d", " ", "-f2"], "abc\na b\n");
        assert_eq!(out.stdout, "b\n");
    }

    #[test]
    fn picks_characters() {
        assert_eq!(
            run("cut", &["-c", "1-3,5"], "héllo world\n").stdout,
            "hélo\n"
        );
        assert_eq!(run("cut", &["-c-2"], "abc\nxyz\n").stdout, "ab\nxy\n");
    }

    #[test]
    fn rejects_bad_lists() {
        for (args, message) in [
            (&["-f", "0"][..], "numbered from 1"),
            (&["-f", "3-1"], "decreasing range"),
            (&["-f", "x"], "invalid field value 'x'"),
            (&["a.txt"], "specify a list"),
            (&["-f1", "-c1"], "only one list"),
            (&["-d", "::", "-f1"], "single character"),
        ] {
            let out = run("cut", args, "");
            assert_eq!(out.status, 1, "{:?}", args);
            assert!(out.stderr.contains(message), "{:?}: {}", args, out.stderr);
        }
    }
}
