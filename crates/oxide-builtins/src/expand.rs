use std::io::{self, BufRead, Write};

use crate::{Ctx, Io};

const DEFAULT_TAB: usize = 8;

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    let (tab, initial_only, files) = match parse_args(args) {
        Ok(parsed) => parsed,
        Err(msg) => {
            ewln!(io, "expand: {}", msg);
            ewln!(io, "Usage: expand [-i] [-t N] [file...]");
            return 1;
        }
    };
    io.each_input("expand", &files, |_, input, out| {
        expand(input, out, tab, initial_only)
    })
}

fn parse_args(args: &[String]) -> Result<(usize, bool, Vec<&str>), String> {
    let mut tab = DEFAULT_TAB;
    let mut initial_only = false;
    let mut files = Vec::new();

    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let value = match arg.as_str() {
            "--" => {
                files.extend(iter.by_ref().map(String::as_str));
                break;
            }
            "-i" => {
                initial_only = true;
                continue;
            }
            "-t" => iter
                .next()
                .map(String::as_str)
                .ok_or("option requires an argument -- 't'")?,
            other => match other.strip_prefix("-t") {
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
        tab = match value.parse() {
            Ok(tab) if tab > 0 => tab,
            _ => return Err(format!("invalid tab size: '{}'", value)),
        };
    }
    Ok((tab, initial_only, files))
}

/// Replaces each tab with the spaces that reach the next multiple of `tab`
/// columns. With `initial_only`, tabs after the first non-blank on a line
/// are left alone.
fn expand(
    input: &mut dyn BufRead,
    out: &mut dyn Write,
    tab: usize,
    initial_only: bool,
) -> io::Result<()> {
    let mut line = String::new();
    loop {
        line.clear();
        if input.read_line(&mut line)? == 0 {
            return Ok(());
        }
        let mut expanded = String::with_capacity(line.len());
        let mut column = 0;
        let mut leading = true;
        for c in line.chars() {
            match c {
                '\t' if leading || !initial_only => {
                    let spaces = tab - column % tab;
                    expanded.extend(std::iter::repeat(' ').take(spaces));
                    column += spaces;
                }
                '\r' | '\n' => {
                    expanded.push(c);
                    column = 0;
                }
                _ => {
                    leading &= c == ' ' || c == '\t';
                    expanded.push(c);
                    column += 1;
                }
            }
        }
        out.write_all(expanded.as_bytes())?;
    }
}

#[cfg(test)]
mod tests {
    use crate::test_support::run;

    #[test]
    fn tabs_reach_the_next_tab_stop() {
        let out = run("expand", &[], "a\tb\n\tc\nabcdefgh\ti\n");
        assert_eq!(out.stdout, "a       b\n        c\nabcdefgh        i\n");
    }

    #[test]
    fn dash_t_sets_the_tab_size() {
        assert_eq!(run("expand", &["-t", "4"], "ab\tc\n").stdout, "ab  c\n");
        assert_eq!(run("expand", &["-t2"], "\t\tx").stdout, "    x");
    }

    #[test]
    fn dash_i_only_expands_leading_tabs() {
        let out = run("expand", &["-i", "-t4"], "\tif\tx\n");
        assert_eq!(out.stdout, "    if\tx\n");
    }

    #[test]
    fn rejects_a_bad_tab_size() {
        let out = run("expand", &["-t", "0"], "");
        assert_eq!(out.status, 1);
        assert!(out.stderr.contains("invalid tab size"));
    }
}
