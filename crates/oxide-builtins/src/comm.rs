use std::cmp::Ordering;
use std::io::BufRead;

use crate::args::parse_flags;
use crate::{Ctx, Io};

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    let (flags, files) = match parse_flags(args, "123") {
        Ok(parsed) => parsed,
        Err(msg) => {
            ewln!(io, "comm: {}", msg);
            ewln!(io, "Usage: comm [-123] FILE1 FILE2");
            return 1;
        }
    };
    if files.len() != 2 {
        ewln!(io, "comm: expected two files, got {}", files.len());
        ewln!(io, "Usage: comm [-123] FILE1 FILE2");
        return 1;
    }

    // Both inputs are read whole: the merge below walks them in step. If
    // either can't be read there is nothing meaningful to compare.
    let mut inputs: Vec<Vec<String>> = Vec::new();
    let status = io.each_input("comm", &files, |_, input, _| {
        inputs.push(input.lines().collect::<Result<_, _>>()?);
        Ok(())
    });
    if status != 0 {
        return status;
    }
    let (left, right) = (&inputs[0], &inputs[1]);

    // Each shown column is indented by one tab per shown column before it.
    let shown = [
        !flags.contains('1'),
        !flags.contains('2'),
        !flags.contains('3'),
    ];
    let indents: Vec<String> = (0..3)
        .map(|col| "\t".repeat(shown[..col].iter().filter(|&&s| s).count()))
        .collect();

    let (mut i, mut j) = (0, 0);
    loop {
        let (col, line) = match (left.get(i), right.get(j)) {
            (None, None) => break,
            (Some(a), Some(b)) => match a.cmp(b) {
                Ordering::Less => {
                    i += 1;
                    (0, a)
                }
                Ordering::Greater => {
                    j += 1;
                    (1, b)
                }
                Ordering::Equal => {
                    i += 1;
                    j += 1;
                    (2, a)
                }
            },
            (Some(a), None) => {
                i += 1;
                (0, a)
            }
            (None, Some(b)) => {
                j += 1;
                (1, b)
            }
        };
        if shown[col] {
            wln!(io, "{}{}", indents[col], line);
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::test_support::{run, scratch_dir};

    #[test]
    fn splits_lines_into_three_columns() {
        let dir = scratch_dir("comm_columns");
        let file = dir.join("b.txt");
        fs::write(&file, "b\nc\nd\n").unwrap();
        let file = file.to_str().unwrap();

        let out = run("comm", &["-", file], "a\nb\nd\ne\n");
        assert_eq!(out.status, 0, "{}", out.stderr);
        assert_eq!(out.stdout, "a\n\t\tb\n\tc\n\t\td\ne\n");

        let out = run("comm", &["-12", "-", file], "a\nb\nd\ne\n");
        assert_eq!(out.stdout, "b\nd\n");

        let out = run("comm", &["-3", "-", file], "a\nb\nd\ne\n");
        assert_eq!(out.stdout, "a\n\tc\ne\n");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn needs_exactly_two_files() {
        let out = run("comm", &["-"], "");
        assert_eq!(out.status, 1);
        assert!(out.stderr.contains("expected two files, got 1"));
    }
}
