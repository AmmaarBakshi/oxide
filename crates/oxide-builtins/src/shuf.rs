use std::collections::HashMap;
use std::io::BufRead;

use crate::mktemp::SplitMix64;
use crate::{Ctx, Io};

/// The most numbers `-i` will shuffle without `-n` to cap them, since all
/// of them have to be held to put them in order.
const MAX_RANGE: u64 = 1 << 24;

/// Where the lines to shuffle come from.
enum Source<'a> {
    /// `-e`: the operands themselves.
    Words(Vec<&'a str>),
    /// `-i LO-HI`: every number in the range.
    Range(u64, u64),
    /// The lines of a file, or stdin.
    Input(Option<&'a str>),
}

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    let (source, count) = match parse_args(args) {
        Ok(parsed) => parsed,
        Err(msg) => {
            ewln!(io, "shuf: {}", msg);
            ewln!(io, "Usage: shuf [-n COUNT] [-e WORD... | -i LO-HI | FILE]");
            return 1;
        }
    };
    let mut rng = SplitMix64::seeded();

    let lines: Vec<String> = match source {
        Source::Range(lo, hi) => {
            let size = hi - lo + 1;
            if count.unwrap_or(size).min(size) > MAX_RANGE {
                ewln!(
                    io,
                    "shuf: range too large to shuffle whole; pick some with -n"
                );
                return 1;
            }
            for i in sample(size, count.unwrap_or(size), &mut rng) {
                wln!(io, "{}", lo + i);
            }
            return 0;
        }
        Source::Words(words) => words.into_iter().map(String::from).collect(),
        Source::Input(file) => {
            let files: Vec<&str> = file.into_iter().collect();
            let mut lines = Vec::new();
            let status = io.each_input("shuf", &files, |_, input, _| {
                for line in input.lines() {
                    lines.push(line?);
                }
                Ok(())
            });
            if status != 0 {
                return status;
            }
            lines
        }
    };
    let total = lines.len() as u64;
    for i in sample(total, count.unwrap_or(total), &mut rng) {
        wln!(io, "{}", lines[i as usize]);
    }
    0
}

fn parse_args(args: &[String]) -> Result<(Source<'_>, Option<u64>), String> {
    let mut count = None;
    let mut range = None;
    let mut words = false;
    let mut operands = Vec::new();

    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--" => {
                operands.extend(iter.by_ref().map(String::as_str));
                break;
            }
            "-e" => words = true,
            "-n" => {
                let value = iter.next().ok_or("option requires an argument -- 'n'")?;
                count = Some(
                    value
                        .parse()
                        .map_err(|_| format!("invalid line count: '{}'", value))?,
                );
            }
            "-i" => {
                let value = iter.next().ok_or("option requires an argument -- 'i'")?;
                range = Some(
                    parse_range(value)
                        .ok_or_else(|| format!("invalid input range: '{}'", value))?,
                );
            }
            // With -e, words that look like options are still words.
            other if !words && other.len() > 1 && other.starts_with('-') => {
                return Err(format!("invalid option '{}'", other));
            }
            operand => operands.push(operand),
        }
    }

    let source = match (words, range) {
        (true, Some(_)) => return Err("cannot combine -e and -i".to_string()),
        (true, None) => Source::Words(operands),
        (false, Some((lo, hi))) => match operands.first() {
            Some(extra) => return Err(format!("extra operand '{}'", extra)),
            None => Source::Range(lo, hi),
        },
        (false, None) => match operands[..] {
            [] => Source::Input(None),
            [file] => Source::Input(Some(file)),
            [_, extra, ..] => return Err(format!("extra operand '{}'", extra)),
        },
    };
    Ok((source, count))
}

/// `LO-HI`, with `LO <= HI` and the whole range countable in a `u64`.
fn parse_range(text: &str) -> Option<(u64, u64)> {
    let (lo, hi) = text.split_once('-')?;
    let (lo, hi): (u64, u64) = (lo.parse().ok()?, hi.parse().ok()?);
    (lo <= hi && (hi - lo).checked_add(1).is_some()).then_some((lo, hi))
}

/// Up to `k` distinct indices below `n`, in random order.
///
/// This is Fisher-Yates over an imaginary array `0..n`, remembering only
/// the slots a swap has touched, so drawing a few numbers from a huge
/// range costs memory only for what is drawn.
fn sample(n: u64, k: u64, rng: &mut SplitMix64) -> Vec<u64> {
    let mut moved: HashMap<u64, u64> = HashMap::new();
    (0..k.min(n))
        .map(|i| {
            let j = i + rng.next_u64() % (n - i);
            let at_i = moved.get(&i).copied().unwrap_or(i);
            let at_j = moved.get(&j).copied().unwrap_or(j);
            moved.insert(j, at_i);
            at_j
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::test_support::run;

    fn sorted(text: &str) -> Vec<String> {
        let mut lines: Vec<String> = text.lines().map(String::from).collect();
        lines.sort();
        lines
    }

    #[test]
    fn prints_every_line_in_some_order() {
        let input: String = (1..=20).map(|n| format!("{}\n", n)).collect();
        let out = run("shuf", &[], &input);
        assert_eq!(out.status, 0, "{}", out.stderr);
        assert_eq!(sorted(&out.stdout), sorted(&input));
    }

    #[test]
    fn the_order_changes_from_run_to_run() {
        let input: String = (1..=100).map(|n| format!("{}\n", n)).collect();
        assert_ne!(
            run("shuf", &[], &input).stdout,
            run("shuf", &[], &input).stdout
        );
    }

    #[test]
    fn dash_n_picks_that_many_distinct_lines() {
        let out = run("shuf", &["-n", "3"], "a\nb\nc\nd\ne\n");
        let mut picked = sorted(&out.stdout);
        assert_eq!(picked.len(), 3);
        picked.dedup();
        assert_eq!(picked.len(), 3);
        assert!(picked.iter().all(|line| "abcde".contains(line.as_str())));

        assert_eq!(
            sorted(&run("shuf", &["-n", "9"], "x\ny\n").stdout),
            ["x", "y"]
        );
    }

    #[test]
    fn dash_e_shuffles_the_operands() {
        let out = run("shuf", &["-e", "a", "-b", "c"], "");
        assert_eq!(sorted(&out.stdout), ["-b", "a", "c"]);
    }

    #[test]
    fn dash_i_shuffles_a_range_of_numbers() {
        let out = run("shuf", &["-i", "1-5"], "");
        assert_eq!(sorted(&out.stdout), ["1", "2", "3", "4", "5"]);
        assert_eq!(run("shuf", &["-i", "7-7"], "").stdout, "7\n");

        // A vast range is fine when only a few numbers are wanted.
        let out = run("shuf", &["-i", "1-1000000000000", "-n", "2"], "");
        let picked: Vec<u64> = out.stdout.lines().map(|n| n.parse().unwrap()).collect();
        assert_eq!(picked.len(), 2);
        assert_ne!(picked[0], picked[1]);
        assert!(picked.iter().all(|&n| (1..=1_000_000_000_000).contains(&n)));
    }

    #[test]
    fn rejects_bad_ranges() {
        for range in ["5-1", "a-b", "7"] {
            let out = run("shuf", &["-i", range], "");
            assert_eq!(out.status, 1, "{}", range);
            assert!(out.stderr.contains("invalid input range"), "{}", out.stderr);
        }
        let out = run("shuf", &["-i", "0-18446744073709551615"], "");
        assert_eq!(out.status, 1);
    }
}
