use std::io::BufRead;

use crate::{Ctx, Io};

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    let (delimiters, serial, files) = match parse_args(args) {
        Ok(parsed) => parsed,
        Err(msg) => {
            ewln!(io, "paste: {}", msg);
            ewln!(io, "Usage: paste [-s] [-d LIST] [file...]");
            return 1;
        }
    };

    // Every input is read whole up front: side by side, line N of the last
    // file is needed before line N+1 of the first. An unreadable file is
    // reported and contributes no lines.
    let mut columns: Vec<Vec<String>> = Vec::new();
    let status = io.each_input("paste", &files, |_, input, _| {
        let lines = input.lines().collect::<Result<Vec<_>, _>>()?;
        columns.push(lines);
        Ok(())
    });

    let rows: Vec<Vec<&str>> = if serial {
        // `-s` turns each file into one row.
        columns
            .iter()
            .map(|lines| lines.iter().map(String::as_str).collect())
            .collect()
    } else {
        let height = columns.iter().map(Vec::len).max().unwrap_or(0);
        (0..height)
            .map(|row| {
                columns
                    .iter()
                    .map(|lines| lines.get(row).map_or("", String::as_str))
                    .collect()
            })
            .collect()
    };

    for row in rows {
        let mut line = String::new();
        for (i, cell) in row.iter().enumerate() {
            if i > 0 {
                // The delimiters are used in turn, wrapping round.
                line.push_str(&delimiters[(i - 1) % delimiters.len()]);
            }
            line.push_str(cell);
        }
        wln!(io, "{}", line);
    }
    status
}

/// Splits out `-s` and `-d LIST`. The list is broken into one delimiter
/// per character, with `\t`, `\n`, `\\` and `\0` (no delimiter) escapes.
fn parse_args(args: &[String]) -> Result<(Vec<String>, bool, Vec<&str>), String> {
    let mut delimiters = vec!["\t".to_string()];
    let mut serial = false;
    let mut files = Vec::new();

    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let list = match arg.as_str() {
            "--" => {
                files.extend(iter.by_ref().map(String::as_str));
                break;
            }
            "-s" => {
                serial = true;
                continue;
            }
            "-d" => iter
                .next()
                .map(String::as_str)
                .ok_or("option requires an argument -- 'd'")?,
            other => match other.strip_prefix("-d") {
                Some(list) => list,
                None if other.len() > 1 && other.starts_with('-') => {
                    return Err(format!("invalid option '{}'", other));
                }
                None => {
                    files.push(other);
                    continue;
                }
            },
        };
        delimiters = split_delimiters(list);
        if delimiters.is_empty() {
            delimiters.push(String::new());
        }
    }
    Ok((delimiters, serial, files))
}

fn split_delimiters(list: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = list.chars();
    while let Some(c) = chars.next() {
        let delimiter = match (c, c == '\\') {
            (_, true) => match chars.next() {
                Some('t') => "\t".to_string(),
                Some('n') => "\n".to_string(),
                Some('0') | None => String::new(),
                Some(other) => other.to_string(),
            },
            (c, false) => c.to_string(),
        };
        out.push(delimiter);
    }
    out
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::test_support::{run, scratch_dir};

    #[test]
    fn joins_files_side_by_side() {
        let dir = scratch_dir("paste_side");
        let (a, b) = (dir.join("a.txt"), dir.join("b.txt"));
        fs::write(&a, "1\n2\n3\n").unwrap();
        fs::write(&b, "x\ny\n").unwrap();

        let out = run("paste", &[a.to_str().unwrap(), b.to_str().unwrap()], "");
        assert_eq!(out.status, 0, "{}", out.stderr);
        assert_eq!(out.stdout, "1\tx\n2\ty\n3\t\n");

        let out = run(
            "paste",
            &["-d", ",;", a.to_str().unwrap(), "-", b.to_str().unwrap()],
            "s\n",
        );
        assert_eq!(out.stdout, "1,s;x\n2,;y\n3,;\n");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn dash_s_joins_each_input_into_one_line() {
        assert_eq!(run("paste", &["-s", "-d,"], "1\n2\n3\n").stdout, "1,2,3\n");
        assert_eq!(run("paste", &["-s"], "a\nb\n").stdout, "a\tb\n");
    }

    #[test]
    fn rejects_unknown_flags() {
        let out = run("paste", &["-x"], "");
        assert_eq!(out.status, 1);
        assert!(out.stderr.contains("invalid option '-x'"));
    }
}
