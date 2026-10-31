use std::io::{self, BufRead, Write};

use crate::{Ctx, Io};

const DEFAULT_TAB: usize = 8;

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    let (tab, all, files) = match parse_args(args) {
        Ok(parsed) => parsed,
        Err(msg) => {
            ewln!(io, "unexpand: {}", msg);
            ewln!(io, "Usage: unexpand [-a] [-t N] [file...]");
            return 1;
        }
    };
    io.each_input("unexpand", &files, |_, input, out| {
        unexpand(input, out, tab, all)
    })
}

/// Like `expand`'s parser, except that `-t` also turns on `-a`, as it does
/// in coreutils: asking for a tab size means converting every run of blanks.
fn parse_args(args: &[String]) -> Result<(usize, bool, Vec<&str>), String> {
    let mut tab = DEFAULT_TAB;
    let mut all = false;
    let mut files = Vec::new();

    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let value = match arg.as_str() {
            "--" => {
                files.extend(iter.by_ref().map(String::as_str));
                break;
            }
            "-a" => {
                all = true;
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
        all = true;
    }
    Ok((tab, all, files))
}

/// Replaces runs of spaces that end on a tab stop with a tab. Without `all`,
/// only the blanks before the first non-blank on a line are touched.
///
/// A lone space that happens to reach a tab stop is left as a space, since a
/// tab there would save nothing and would change how the text is split by
/// tools that look for spaces.
fn unexpand(input: &mut dyn BufRead, out: &mut dyn Write, tab: usize, all: bool) -> io::Result<()> {
    let mut line = String::new();
    loop {
        line.clear();
        if input.read_line(&mut line)? == 0 {
            return Ok(());
        }
        let mut packed = String::with_capacity(line.len());
        let mut column = 0;
        let mut pending = 0;
        let mut leading = true;
        for c in line.chars() {
            let convert = all || leading;
            match c {
                ' ' if convert => {
                    column += 1;
                    pending += 1;
                    if column % tab == 0 {
                        packed.push(if pending > 1 { '\t' } else { ' ' });
                        pending = 0;
                    }
                }
                '\t' if convert => {
                    // The tab covers any spaces waiting before it.
                    column += tab - column % tab;
                    pending = 0;
                    packed.push('\t');
                }
                _ => {
                    packed.extend(std::iter::repeat_n(' ', pending));
                    pending = 0;
                    match c {
                        '\t' => column += tab - column % tab,
                        '\r' | '\n' => column = 0,
                        ' ' => column += 1,
                        _ => {
                            leading = false;
                            column += 1;
                        }
                    }
                    packed.push(c);
                }
            }
        }
        packed.extend(std::iter::repeat_n(' ', pending));
        out.write_all(packed.as_bytes())?;
    }
}

#[cfg(test)]
mod tests {
    use crate::test_support::run;

    #[test]
    fn leading_spaces_become_tabs() {
        let out = run("unexpand", &[], "        x\n          y\n");
        assert_eq!(out.stdout, "\tx\n\t  y\n");
    }

    #[test]
    fn interior_spaces_need_dash_a() {
        let line = "a       b\n";
        assert_eq!(run("unexpand", &[], line).stdout, line);
        assert_eq!(run("unexpand", &["-a"], line).stdout, "a\tb\n");
    }

    #[test]
    fn a_lone_space_before_a_tab_stop_stays() {
        let line = "abcdefg h\n";
        assert_eq!(run("unexpand", &["-a"], line).stdout, line);
    }

    #[test]
    fn dash_t_sets_the_size_and_implies_dash_a() {
        let out = run("unexpand", &["-t4"], "    ab  c\n");
        assert_eq!(out.stdout, "\tab\tc\n");
        let out = run("unexpand", &["-t", "4"], "ab   \n");
        assert_eq!(out.stdout, "ab\t \n");
    }

    #[test]
    fn a_tab_swallows_the_spaces_before_it() {
        assert_eq!(run("unexpand", &[], "  \tx\n").stdout, "\tx\n");
    }

    #[test]
    fn rejects_a_bad_tab_size() {
        let out = run("unexpand", &["-t", "0"], "");
        assert_eq!(out.status, 1);
        assert!(out.stderr.contains("invalid tab size"));
    }
}
