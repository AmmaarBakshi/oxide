use sysinfo::System;

use crate::args::parse_flags;
use crate::{Ctx, Io};

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    let (flags, operands) = match parse_flags(args, "p") {
        Ok(parsed) => parsed,
        Err(msg) => {
            ewln!(io, "uptime: {}", msg);
            ewln!(io, "Usage: uptime [-p]");
            return 1;
        }
    };
    if let Some(extra) = operands.first() {
        ewln!(io, "uptime: extra operand '{}'", extra);
        return 1;
    }

    let secs = System::uptime();
    if flags.contains('p') {
        wln!(io, "{}", pretty(secs));
        return 0;
    }
    // Windows keeps no load average (sysinfo reports zeros), so rather
    // than print a misleading idle system, leave it out there.
    if cfg!(windows) {
        wln!(io, "{}", short(secs));
    } else {
        let load = System::load_average();
        wln!(
            io,
            "{},  load average: {:.2}, {:.2}, {:.2}",
            short(secs),
            load.one,
            load.five,
            load.fifteen
        );
    }
    0
}

/// `up 3 days, 4:05`, or `up 12 min` in the first hour, as procps prints it.
fn short(secs: u64) -> String {
    let (days, hours, minutes) = split(secs);
    let mut text = String::from("up ");
    if days > 0 {
        text += &format!("{} day{}, ", days, plural(days));
    }
    if hours > 0 {
        text += &format!("{}:{:02}", hours, minutes);
    } else {
        text += &format!("{} min", minutes);
    }
    text
}

/// `up 3 days, 4 hours, 5 minutes`, leaving out any part that's zero.
fn pretty(secs: u64) -> String {
    let (days, hours, minutes) = split(secs);
    let parts: Vec<String> = [(days, "day"), (hours, "hour"), (minutes, "minute")]
        .into_iter()
        .filter(|&(n, _)| n > 0)
        .map(|(n, unit)| format!("{} {}{}", n, unit, plural(n)))
        .collect();
    if parts.is_empty() {
        "up 0 minutes".to_string()
    } else {
        format!("up {}", parts.join(", "))
    }
}

fn split(secs: u64) -> (u64, u64, u64) {
    (secs / 86_400, secs % 86_400 / 3_600, secs % 3_600 / 60)
}

fn plural(n: u64) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}

#[cfg(test)]
mod tests {
    use super::{pretty, short};
    use crate::test_support::run;

    const MINUTE: u64 = 60;
    const HOUR: u64 = 60 * MINUTE;
    const DAY: u64 = 24 * HOUR;

    #[test]
    fn short_form_shows_hours_and_minutes() {
        assert_eq!(short(12 * MINUTE + 30), "up 12 min");
        assert_eq!(short(4 * HOUR + 5 * MINUTE), "up 4:05");
        assert_eq!(short(DAY + 10 * HOUR), "up 1 day, 10:00");
        assert_eq!(short(3 * DAY + 7 * MINUTE), "up 3 days, 7 min");
    }

    #[test]
    fn pretty_form_spells_out_each_unit() {
        assert_eq!(pretty(30), "up 0 minutes");
        assert_eq!(pretty(MINUTE), "up 1 minute");
        assert_eq!(
            pretty(2 * DAY + HOUR + 5 * MINUTE),
            "up 2 days, 1 hour, 5 minutes"
        );
        assert_eq!(pretty(DAY + 3 * MINUTE), "up 1 day, 3 minutes");
    }

    #[test]
    fn reports_how_long_the_system_has_been_up() {
        for args in [&[][..], &["-p"]] {
            let out = run("uptime", args, "");
            assert_eq!(out.status, 0, "{}", out.stderr);
            assert!(out.stdout.starts_with("up "), "{}", out.stdout);
        }
    }

    #[test]
    fn takes_no_operands() {
        let out = run("uptime", &["now"], "");
        assert_eq!(out.status, 1);
        assert!(out.stderr.contains("extra operand 'now'"));
    }
}
