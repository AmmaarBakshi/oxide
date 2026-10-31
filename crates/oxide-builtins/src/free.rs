use sysinfo::System;

use crate::args::parse_flags;
use crate::{Ctx, Io};

/// Memory figures, all in bytes.
struct Memory {
    total: u64,
    used: u64,
    free: u64,
    available: u64,
    swap_total: u64,
    swap_used: u64,
    swap_free: u64,
}

#[derive(Clone, Copy)]
enum Unit {
    Bytes,
    Kibi,
    Mebi,
    Gibi,
    Human,
}

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    let (flags, operands) = match parse_flags(args, "bkmgh") {
        Ok(parsed) => parsed,
        Err(msg) => {
            ewln!(io, "free: {}", msg);
            ewln!(io, "Usage: free [-b | -k | -m | -g | -h]");
            return 1;
        }
    };
    if let Some(extra) = operands.first() {
        ewln!(io, "free: extra operand '{}'", extra);
        return 1;
    }
    // Kibibytes by default, as procps prints; the last unit flag wins.
    let unit = match flags.chars().last() {
        Some('b') => Unit::Bytes,
        Some('m') => Unit::Mebi,
        Some('g') => Unit::Gibi,
        Some('h') => Unit::Human,
        _ => Unit::Kibi,
    };

    let mut sys = System::new();
    sys.refresh_memory();
    let memory = Memory {
        total: sys.total_memory(),
        used: sys.used_memory(),
        free: sys.free_memory(),
        available: sys.available_memory(),
        swap_total: sys.total_swap(),
        swap_used: sys.used_swap(),
        swap_free: sys.free_swap(),
    };
    for line in table(&memory, unit) {
        wln!(io, "{}", line);
    }
    0
}

fn table(m: &Memory, unit: Unit) -> [String; 3] {
    let f = |bytes| scaled(bytes, unit);
    [
        format!(
            "{:5}{:>12}{:>12}{:>12}{:>12}",
            "", "total", "used", "free", "available"
        ),
        format!(
            "{:5}{:>12}{:>12}{:>12}{:>12}",
            "Mem:",
            f(m.total),
            f(m.used),
            f(m.free),
            f(m.available)
        ),
        format!(
            "{:5}{:>12}{:>12}{:>12}",
            "Swap:",
            f(m.swap_total),
            f(m.swap_used),
            f(m.swap_free)
        ),
    ]
}

fn scaled(bytes: u64, unit: Unit) -> String {
    match unit {
        Unit::Bytes => bytes.to_string(),
        Unit::Kibi => (bytes >> 10).to_string(),
        Unit::Mebi => (bytes >> 20).to_string(),
        Unit::Gibi => (bytes >> 30).to_string(),
        Unit::Human => human(bytes),
    }
}

/// A byte count the way coreutils' `-h` shows it: `1023`, `1.5K`, `20M`.
/// Under ten units one decimal is kept; above, whole units. Both round up,
/// so a size is never shown as smaller than it is.
pub(crate) fn human(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["K", "M", "G", "T", "P", "E"];
    if bytes < 1024 {
        return bytes.to_string();
    }
    let mut value = bytes as f64 / 1024.0;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if value < 10.0 {
        let tenths = (value * 10.0).ceil() / 10.0;
        if tenths < 10.0 {
            return format!("{:.1}{}", tenths, UNITS[unit]);
        }
        value = tenths;
    }
    let whole = value.ceil();
    if whole >= 1024.0 && unit + 1 < UNITS.len() {
        format!("1.0{}", UNITS[unit + 1])
    } else {
        format!("{}{}", whole, UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::{human, table, Memory, Unit};
    use crate::test_support::run;

    const GIB: u64 = 1 << 30;

    fn sample() -> Memory {
        Memory {
            total: 16 * GIB,
            used: 6 * GIB,
            free: 2 * GIB,
            available: 10 * GIB,
            swap_total: 4 * GIB,
            swap_used: 0,
            swap_free: 4 * GIB,
        }
    }

    #[test]
    fn lays_out_memory_and_swap_in_columns() {
        let [header, mem, swap] = table(&sample(), Unit::Gibi);
        assert_eq!(
            header,
            "            total        used        free   available"
        );
        assert_eq!(mem, "Mem:           16           6           2          10");
        assert_eq!(swap, "Swap:           4           0           4");
    }

    #[test]
    fn units_scale_the_figures() {
        let total_in = |unit| {
            table(&sample(), unit)[1]
                .split_whitespace()
                .nth(1)
                .unwrap()
                .to_string()
        };
        assert_eq!(total_in(Unit::Bytes), (16 * GIB).to_string());
        assert_eq!(total_in(Unit::Kibi), (16 << 20).to_string());
        assert_eq!(total_in(Unit::Mebi), "16384");
        assert_eq!(total_in(Unit::Human), "16G");
    }

    #[test]
    fn human_sizes_round_up() {
        assert_eq!(human(0), "0");
        assert_eq!(human(1023), "1023");
        assert_eq!(human(1024), "1.0K");
        assert_eq!(human(1025), "1.1K");
        assert_eq!(human(1536), "1.5K");
        assert_eq!(human(10 * 1024 - 1), "10K");
        assert_eq!(human(10 * 1024), "10K");
        assert_eq!(human(1024 * 1024 - 1), "1.0M");
        assert_eq!(human(3 * GIB / 2), "1.5G");
    }

    #[test]
    fn reports_this_machine() {
        let out = run("free", &["-m"], "");
        assert_eq!(out.status, 0, "{}", out.stderr);
        let labels: Vec<&str> = out
            .stdout
            .lines()
            .filter_map(|line| line.split_whitespace().next())
            .collect();
        assert_eq!(labels, ["total", "Mem:", "Swap:"]);
    }
}
