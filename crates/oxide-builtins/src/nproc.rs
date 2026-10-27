use std::thread;

use crate::{Ctx, Io};

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    let ignore = match parse_args(args) {
        Ok(ignore) => ignore,
        Err(msg) => {
            ewln!(io, "nproc: {}", msg);
            ewln!(io, "Usage: nproc [--ignore N]");
            return 1;
        }
    };
    let available = thread::available_parallelism().map_or(1, |n| n.get());
    wln!(io, "{}", count(available, ignore));
    0
}

/// Reads `--ignore N` (or `--ignore=N`), the number of processors to leave
/// out of the count.
fn parse_args(args: &[String]) -> Result<usize, String> {
    let mut ignore = 0;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let value = match arg.as_str() {
            "--ignore" => iter
                .next()
                .map(String::as_str)
                .ok_or("option '--ignore' requires an argument")?,
            other => match other.strip_prefix("--ignore=") {
                Some(value) => value,
                None if other.starts_with('-') => {
                    return Err(format!("invalid option '{}'", other));
                }
                None => return Err(format!("extra operand '{}'", other)),
            },
        };
        ignore = value
            .parse()
            .map_err(|_| format!("invalid number: '{}'", value))?;
    }
    Ok(ignore)
}

/// The processors left after ignoring some, never fewer than one: there is
/// always at least the one running this.
fn count(available: usize, ignore: usize) -> usize {
    available.saturating_sub(ignore).max(1)
}

#[cfg(test)]
mod tests {
    use crate::test_support::run;

    #[test]
    fn prints_a_positive_count() {
        let out = run("nproc", &[], "");
        assert_eq!(out.status, 0, "{}", out.stderr);
        assert!(out.stdout.trim().parse::<usize>().unwrap() >= 1);
    }

    #[test]
    fn ignore_never_drops_below_one() {
        assert_eq!(super::count(8, 2), 6);
        assert_eq!(super::count(4, 10), 1);
        assert_eq!(run("nproc", &["--ignore=100000"], "").stdout, "1\n");
        assert_eq!(run("nproc", &["--ignore", "100000"], "").stdout, "1\n");
    }

    #[test]
    fn rejects_bad_arguments() {
        let out = run("nproc", &["--ignore", "x"], "");
        assert_eq!(out.status, 1);
        assert!(out.stderr.contains("invalid number: 'x'"));

        let out = run("nproc", &["4"], "");
        assert_eq!(out.status, 1);
        assert!(out.stderr.contains("extra operand '4'"));
    }
}
