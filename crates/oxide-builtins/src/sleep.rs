use std::thread;
use std::time::Duration;

use crate::{Ctx, Io};

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    if args.is_empty() {
        ewln!(io, "oxide: sleep: missing operand (e.g., 'sleep 5' or 'sleep 0.5')");
        return 1;
    }

    // Fractional seconds are allowed; negative, NaN and absurdly large values
    // are rejected by `try_from_secs_f64` rather than panicking.
    let duration = args[0]
        .parse::<f64>()
        .ok()
        .and_then(|secs| Duration::try_from_secs_f64(secs).ok());

    if let Some(duration) = duration {
        thread::sleep(duration);
        0
    } else {
        ewln!(io, "oxide: sleep: invalid time interval '{}'", args[0]);
        1
    }
}

#[cfg(test)]
mod tests {
    use crate::test_support::run;

    #[test]
    fn accepts_fractional_seconds() {
        assert_eq!(run("sleep", &["0.01"], "").status, 0);
    }

    #[test]
    fn rejects_invalid_intervals() {
        for bad in ["-1", "soon", "NaN", "inf"] {
            let out = run("sleep", &[bad], "");
            assert_eq!(out.status, 1, "accepted '{}'", bad);
            assert!(out.stderr.contains("invalid time interval"));
        }
    }
}
