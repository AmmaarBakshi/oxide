use std::thread;
use std::time::Duration;

use crate::{Ctx, Io};

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    if args.is_empty() {
        ewln!(
            io,
            "oxide: sleep: missing operand (e.g., 'sleep 5' or 'sleep 0.5')"
        );
        return 1;
    }

    // Like coreutils, several operands are added together, so `sleep 1m 30s`
    // waits ninety seconds. Every operand is checked before sleeping at all.
    let mut total = Duration::ZERO;
    for arg in args {
        let Some(duration) = parse_interval(arg) else {
            ewln!(io, "oxide: sleep: invalid time interval '{}'", arg);
            return 1;
        };
        let Some(sum) = total.checked_add(duration) else {
            ewln!(io, "oxide: sleep: time interval too large");
            return 1;
        };
        total = sum;
    }

    thread::sleep(total);
    0
}

/// Parses a number of seconds with an optional `s`, `m`, `h` or `d` suffix.
/// Fractional values are allowed; negative, NaN and absurdly large values
/// are rejected by `try_from_secs_f64` rather than panicking.
fn parse_interval(arg: &str) -> Option<Duration> {
    let (number, scale) = match arg.char_indices().last()? {
        (i, 's') => (&arg[..i], 1.0),
        (i, 'm') => (&arg[..i], 60.0),
        (i, 'h') => (&arg[..i], 60.0 * 60.0),
        (i, 'd') => (&arg[..i], 24.0 * 60.0 * 60.0),
        _ => (arg, 1.0),
    };
    let secs = number.parse::<f64>().ok()?;
    Duration::try_from_secs_f64(secs * scale).ok()
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::parse_interval;
    use crate::test_support::run;

    #[test]
    fn accepts_fractional_seconds() {
        assert_eq!(run("sleep", &["0.01"], "").status, 0);
    }

    #[test]
    fn adds_up_every_operand() {
        assert_eq!(run("sleep", &["0.01", "0.01s"], "").status, 0);
    }

    #[test]
    fn suffixes_scale_the_number() {
        assert_eq!(parse_interval("1.5"), Some(Duration::from_millis(1500)));
        assert_eq!(parse_interval("2s"), Some(Duration::from_secs(2)));
        assert_eq!(parse_interval("0.5m"), Some(Duration::from_secs(30)));
        assert_eq!(parse_interval("1h"), Some(Duration::from_secs(3600)));
        assert_eq!(parse_interval("1d"), Some(Duration::from_secs(86400)));
    }

    #[test]
    fn rejects_invalid_intervals() {
        for bad in ["-1", "soon", "NaN", "inf", "", "s", "5x", "1e400"] {
            let out = run("sleep", &[bad], "");
            assert_eq!(out.status, 1, "accepted '{}'", bad);
            assert!(out.stderr.contains("invalid time interval"));
        }
    }

    #[test]
    fn a_bad_operand_anywhere_means_no_sleep() {
        // Fails fast instead of sleeping an hour before noticing 'later'.
        let out = run("sleep", &["1h", "later"], "");
        assert_eq!(out.status, 1);
        assert!(out.stderr.contains("'later'"));
    }
}
