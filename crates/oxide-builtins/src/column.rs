use std::env;
use std::io::BufRead;

use crate::{Ctx, Io};

const DEFAULT_WIDTH: usize = 80;

/// Space between columns in fill mode.
const GAP: usize = 2;

struct Options<'a> {
    /// `-t`: line up fields into a table rather than filling columns.
    table: bool,
    /// `-s`: the characters that separate fields in table mode.
    separators: Option<&'a str>,
    /// `-o`: what goes between table columns.
    output_sep: &'a str,
    /// `-c`: the width to fill.
    width: usize,
    /// `-x`: fill rows before columns.
    across: bool,
}

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    let (opts, files) = match parse_args(args) {
        Ok(parsed) => parsed,
        Err(msg) => {
            ewln!(io, "column: {}", msg);
            ewln!(
                io,
                "Usage: column [-t [-s SEP] [-o SEP]] [-x] [-c WIDTH] [file...]"
            );
            return 1;
        }
    };

    // Every line is needed before the first can be laid out. Blank lines
    // are dropped, as util-linux does.
    let mut lines = Vec::new();
    let status = io.each_input("column", &files, |_, input, _| {
        for line in input.lines() {
            let line = line?;
            if !line.trim().is_empty() {
                lines.push(line);
            }
        }
        Ok(())
    });

    let output = if opts.table {
        table(&lines, &opts)
    } else {
        fill(&lines, opts.width, opts.across)
    };
    for line in output {
        wln!(io, "{}", line);
    }
    status
}

fn parse_args(args: &[String]) -> Result<(Options<'_>, Vec<&str>), String> {
    let mut opts = Options {
        table: false,
        separators: None,
        output_sep: "  ",
        width: env::var("COLUMNS")
            .ok()
            .and_then(|cols| cols.parse().ok())
            .unwrap_or(DEFAULT_WIDTH),
        across: false,
    };
    let mut files = Vec::new();

    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let arg = arg.as_str();
        match arg {
            "--" => {
                files.extend(iter.by_ref().map(String::as_str));
                break;
            }
            "-t" => opts.table = true,
            "-x" => opts.across = true,
            "-s" | "-o" | "-c" => {
                let value = iter
                    .next()
                    .ok_or_else(|| format!("option requires an argument -- '{}'", &arg[1..]))?;
                match arg {
                    "-s" => opts.separators = Some(value),
                    "-o" => opts.output_sep = value,
                    _ => {
                        opts.width = value
                            .parse()
                            .map_err(|_| format!("invalid width: '{}'", value))?;
                    }
                }
            }
            other if other.len() > 1 && other.starts_with('-') => {
                return Err(format!("invalid option '{}'", other));
            }
            file => files.push(file),
        }
    }
    Ok((opts, files))
}

/// Splits each line into fields and pads every column to its widest
/// cell. The last column isn't padded, so lines carry no trailing blanks.
fn table(lines: &[String], opts: &Options<'_>) -> Vec<String> {
    let rows: Vec<Vec<&str>> = lines
        .iter()
        .map(|line| match opts.separators {
            Some(seps) => line.split(|c| seps.contains(c)).collect(),
            None => line.split_whitespace().collect(),
        })
        .collect();
    let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
    let widths: Vec<usize> = (0..columns)
        .map(|col| {
            rows.iter()
                .filter_map(|row| row.get(col))
                .map(|cell| cell.chars().count())
                .max()
                .unwrap_or(0)
        })
        .collect();
    rows.iter()
        .map(|row| {
            let mut line = String::new();
            for (col, cell) in row.iter().enumerate() {
                if col > 0 {
                    line.push_str(opts.output_sep);
                }
                line.push_str(cell);
                if col + 1 < row.len() {
                    let pad = widths[col] - cell.chars().count();
                    line.extend(std::iter::repeat_n(' ', pad));
                }
            }
            line
        })
        .collect()
}

/// Arranges the entries in as many columns as fit in `width`, filling
/// each column top to bottom, or each row left to right with `across`.
fn fill(entries: &[String], width: usize, across: bool) -> Vec<String> {
    if entries.is_empty() {
        return Vec::new();
    }
    let widest = entries.iter().map(|e| e.chars().count()).max().unwrap_or(0);
    let column_width = widest + GAP;
    let columns = (width / column_width).clamp(1, entries.len());
    let rows = entries.len().div_ceil(columns);

    (0..rows)
        .map(|row| {
            let cells: Vec<&String> = (0..columns)
                .map(|col| {
                    if across {
                        row * columns + col
                    } else {
                        col * rows + row
                    }
                })
                .filter_map(|i| entries.get(i))
                .collect();
            let mut line = String::new();
            for (i, cell) in cells.iter().enumerate() {
                line.push_str(cell);
                if i + 1 < cells.len() {
                    line.extend(std::iter::repeat_n(
                        ' ',
                        column_width - cell.chars().count(),
                    ));
                }
            }
            line
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::test_support::run;

    #[test]
    fn dash_t_lines_fields_up_in_a_table() {
        let out = run("column", &["-t"], "name age\nalice 30\n\nbob 4\n");
        assert_eq!(out.status, 0, "{}", out.stderr);
        assert_eq!(out.stdout, "name   age\nalice  30\nbob    4\n");
    }

    #[test]
    fn separators_can_be_chosen() {
        let out = run(
            "column",
            &["-t", "-s", ",", "-o", " | "],
            "a,bb,c\nddd,,f\n",
        );
        assert_eq!(out.stdout, "a   | bb | c\nddd |    | f\n");
    }

    #[test]
    fn fills_columns_to_the_width() {
        let input = "one\ntwo\nthree\nfour\nfive\n";
        assert_eq!(
            run("column", &["-c", "20"], input).stdout,
            "one    four\ntwo    five\nthree\n"
        );
        assert_eq!(
            run("column", &["-x", "-c", "20"], input).stdout,
            "one    two\nthree  four\nfive\n"
        );
        // Too narrow for two columns still gives one.
        assert_eq!(run("column", &["-c", "1"], "ab\ncd\n").stdout, "ab\ncd\n");
    }

    #[test]
    fn rejects_a_bad_width() {
        let out = run("column", &["-c", "wide"], "");
        assert_eq!(out.status, 1);
        assert!(out.stderr.contains("invalid width: 'wide'"));
    }
}
