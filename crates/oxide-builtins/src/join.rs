use std::cmp::Ordering;
use std::io::BufRead;

use crate::{Ctx, Io};

struct Options {
    /// `-t`: the field separator. Without one, fields are split on runs of
    /// blanks and joined with a single space.
    sep: Option<char>,
    /// `-1`/`-2`: the join field of each file, counted from 0.
    fields: [usize; 2],
    /// `-a`/`-v`: also print the lines of each file that pair with nothing.
    unpaired: [bool; 2],
    /// `-v`: print only those, not the joined lines.
    only_unpaired: bool,
}

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    let (opts, files) = match parse_args(args) {
        Ok(parsed) => parsed,
        Err(msg) => {
            ewln!(io, "join: {}", msg);
            ewln!(
                io,
                "Usage: join [-t CHAR] [-1 F] [-2 F] [-a N] [-v N] FILE1 FILE2"
            );
            return 1;
        }
    };

    let mut inputs: Vec<Vec<String>> = Vec::new();
    let status = io.each_input("join", &files, |_, input, _| {
        inputs.push(input.lines().collect::<Result<_, _>>()?);
        Ok(())
    });
    if status != 0 {
        return status;
    }
    let split = |line: &'_ str| -> Vec<String> {
        match opts.sep {
            Some(sep) => line.split(sep).map(String::from).collect(),
            None => line.split_whitespace().map(String::from).collect(),
        }
    };
    let rows: Vec<Vec<Vec<String>>> = inputs
        .iter()
        .map(|lines| lines.iter().map(|line| split(line)).collect())
        .collect();
    let (a, b) = (&rows[0], &rows[1]);
    let key = |side: usize, row: &[String]| row.get(opts.fields[side]).cloned().unwrap_or_default();

    // Both files are sorted on their join field, so walk them in step:
    // lines with equal keys pair up, every line of one group with every
    // line of the other.
    let (mut i, mut j) = (0, 0);
    while i < a.len() || j < b.len() {
        let order = match (a.get(i), b.get(j)) {
            (Some(x), Some(y)) => key(0, x).cmp(&key(1, y)),
            (Some(_), None) => Ordering::Less,
            _ => Ordering::Greater,
        };
        match order {
            Ordering::Less => {
                if opts.unpaired[0] {
                    wln!(io, "{}", format_row(&opts, &key(0, &a[i]), &[(&a[i], 0)]));
                }
                i += 1;
            }
            Ordering::Greater => {
                if opts.unpaired[1] {
                    wln!(io, "{}", format_row(&opts, &key(1, &b[j]), &[(&b[j], 1)]));
                }
                j += 1;
            }
            Ordering::Equal => {
                let k = key(0, &a[i]);
                let i_end = i + a[i..].iter().take_while(|row| key(0, row) == k).count();
                let j_end = j + b[j..].iter().take_while(|row| key(1, row) == k).count();
                if !opts.only_unpaired {
                    for x in &a[i..i_end] {
                        for y in &b[j..j_end] {
                            wln!(io, "{}", format_row(&opts, &k, &[(x, 0), (y, 1)]));
                        }
                    }
                }
                (i, j) = (i_end, j_end);
            }
        }
    }
    0
}

/// The join field first, then each row's other fields in turn.
fn format_row(opts: &Options, key: &str, rows: &[(&Vec<String>, usize)]) -> String {
    let mut fields = vec![key];
    for &(row, side) in rows {
        fields.extend(
            row.iter()
                .enumerate()
                .filter(|&(n, _)| n != opts.fields[side])
                .map(|(_, field)| field.as_str()),
        );
    }
    let sep = opts.sep.map_or(" ".to_string(), String::from);
    fields.join(&sep)
}

fn parse_args(args: &[String]) -> Result<(Options, Vec<&str>), String> {
    let mut opts = Options {
        sep: None,
        fields: [0, 0],
        unpaired: [false, false],
        only_unpaired: false,
    };
    let mut files = Vec::new();

    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let arg = arg.as_str();
        if let Some(attached) = arg.strip_prefix("-t") {
            let value = match attached {
                "" => iter.next().ok_or("option requires an argument -- 't'")?,
                value => value,
            };
            let mut chars = value.chars();
            match (chars.next(), chars.next()) {
                (Some(sep), None) => opts.sep = Some(sep),
                _ => return Err(format!("the separator must be one character: '{}'", value)),
            }
            continue;
        }
        let option = match arg {
            "--" => {
                files.extend(iter.by_ref().map(String::as_str));
                break;
            }
            "-1" | "-2" | "-j" | "-a" | "-v" => arg,
            other if other.len() > 1 && other.starts_with('-') => {
                return Err(format!("invalid option '{}'", other));
            }
            file => {
                files.push(file);
                continue;
            }
        };
        let value = iter
            .next()
            .ok_or_else(|| format!("option requires an argument -- '{}'", &option[1..]))?;
        let n: usize = match value.parse() {
            Ok(n) if n > 0 => n,
            _ => return Err(format!("invalid number: '{}'", value)),
        };
        match option {
            "-1" => opts.fields[0] = n - 1,
            "-2" => opts.fields[1] = n - 1,
            "-j" => opts.fields = [n - 1, n - 1],
            _ => {
                if n > 2 {
                    return Err(format!("invalid file number: '{}'", value));
                }
                opts.unpaired[n - 1] = true;
                opts.only_unpaired |= option == "-v";
            }
        }
    }
    if files.len() != 2 {
        return Err(format!("expected two files, got {}", files.len()));
    }
    Ok((opts, files))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::test_support::{run, scratch_dir};

    /// Runs `join ARGS FILE1 -` with `left` in FILE1 and `right` on stdin.
    fn join(test: &str, args: &[&str], left: &str, right: &str) -> String {
        let dir = scratch_dir(test);
        let file = dir.join("left.txt");
        fs::write(&file, left).unwrap();
        let mut all = args.to_vec();
        all.extend([file.to_str().unwrap(), "-"]);
        let out = run("join", &all, right);
        assert_eq!(out.status, 0, "{}", out.stderr);
        fs::remove_dir_all(dir).unwrap();
        out.stdout
    }

    const LEFT: &str = "1 a\n2 b\n3 c\n";
    const RIGHT: &str = "1 x\n3 y\n4 z\n";

    #[test]
    fn joins_lines_with_the_same_first_field() {
        assert_eq!(join("join_basic", &[], LEFT, RIGHT), "1 a x\n3 c y\n");
    }

    #[test]
    fn dash_a_and_dash_v_show_unpaired_lines() {
        assert_eq!(
            join("join_all", &["-a", "1", "-a", "2"], LEFT, RIGHT),
            "1 a x\n2 b\n3 c y\n4 z\n"
        );
        assert_eq!(join("join_only", &["-v", "2"], LEFT, RIGHT), "4 z\n");
    }

    #[test]
    fn separator_and_join_fields_can_be_chosen() {
        let out = join(
            "join_fields",
            &["-t", ",", "-1", "2", "-2", "1"],
            "x,apple\ny,pear\n",
            "apple,red\npear,green\n",
        );
        assert_eq!(out, "apple,x,red\npear,y,green\n");
    }

    #[test]
    fn repeated_keys_pair_every_way() {
        let out = join("join_repeats", &[], "k 1\nk 2\n", "k a\nk b\n");
        assert_eq!(out, "k 1 a\nk 1 b\nk 2 a\nk 2 b\n");
    }

    #[test]
    fn needs_exactly_two_files() {
        let out = run("join", &["-"], "");
        assert_eq!(out.status, 1);
        assert!(out.stderr.contains("expected two files, got 1"));
    }
}
