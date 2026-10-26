use std::io::{self, BufRead, Write};

use crate::{Ctx, Io};

const DEFAULT_WIDTH: usize = 80;

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    let (width, at_spaces, files) = match parse_args(args) {
        Ok(parsed) => parsed,
        Err(msg) => {
            ewln!(io, "fold: {}", msg);
            ewln!(io, "Usage: fold [-s] [-w WIDTH] [file...]");
            return 1;
        }
    };
    io.each_input("fold", &files, |_, input, out| {
        fold(input, out, width, at_spaces)
    })
}

fn parse_args(args: &[String]) -> Result<(usize, bool, Vec<&str>), String> {
    let mut width = DEFAULT_WIDTH;
    let mut at_spaces = false;
    let mut files = Vec::new();

    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let value = match arg.as_str() {
            "--" => {
                files.extend(iter.by_ref().map(String::as_str));
                break;
            }
            "-s" => {
                at_spaces = true;
                continue;
            }
            "-w" => iter
                .next()
                .map(String::as_str)
                .ok_or("option requires an argument -- 'w'")?,
            other => match other.strip_prefix("-w") {
                Some(value) => value,
                None if other.len() > 1 && other.starts_with('-') => {
                    return Err(format!("invalid option '{}'", other));
                }
                None => {
                    files.push(other);
                    continue;
                }
            },
        };
        width = match value.parse() {
            Ok(width) if width > 0 => width,
            _ => return Err(format!("invalid number of columns: '{}'", value)),
        };
    }
    Ok((width, at_spaces, files))
}

/// Breaks every line longer than `width` characters. With `at_spaces`, a
/// break goes after the last blank that fits, when there is one.
fn fold(
    input: &mut dyn BufRead,
    out: &mut dyn Write,
    width: usize,
    at_spaces: bool,
) -> io::Result<()> {
    let mut line = String::new();
    loop {
        line.clear();
        if input.read_line(&mut line)? == 0 {
            return Ok(());
        }
        let ending_len = line.len() - line.trim_end_matches(['\n', '\r']).len();
        let (text, ending) = line.split_at(line.len() - ending_len);

        let mut rest: Vec<char> = text.chars().collect();
        while rest.len() > width {
            let mut cut = width;
            if at_spaces {
                if let Some(space) = rest[..width].iter().rposition(|c| c.is_whitespace()) {
                    cut = space + 1;
                }
            }
            let piece: String = rest.drain(..cut).collect();
            writeln!(out, "{}", piece)?;
        }
        let piece: String = rest.into_iter().collect();
        write!(out, "{}{}", piece, ending)?;
    }
}

#[cfg(test)]
mod tests {
    use crate::test_support::run;

    #[test]
    fn breaks_long_lines_at_the_width() {
        let out = run("fold", &["-w", "7"], "the quick brown fox\n");
        assert_eq!(out.stdout, "the qui\nck brow\nn fox\n");
    }

    #[test]
    fn dash_s_breaks_after_spaces() {
        let out = run("fold", &["-s", "-w7"], "the quick brown fox\n");
        assert_eq!(out.stdout, "the \nquick \nbrown \nfox\n");
    }

    #[test]
    fn short_lines_are_untouched() {
        assert_eq!(run("fold", &[], "short\nlines").stdout, "short\nlines");
    }

    #[test]
    fn rejects_a_bad_width() {
        let out = run("fold", &["-w", "0"], "");
        assert_eq!(out.status, 1);
        assert!(out.stderr.contains("invalid number of columns"));
    }
}
