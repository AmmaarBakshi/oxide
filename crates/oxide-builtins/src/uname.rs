use std::env;

use sysinfo::System;

use crate::args::parse_flags;
use crate::{Ctx, Io};

/// Every field, in the order they're printed whatever order they're asked
/// for in, as with coreutils.
const FIELDS: &str = "snrvmo";

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    let (flags, operands) = match parse_flags(args, "asnrvmo") {
        Ok(parsed) => parsed,
        Err(msg) => {
            ewln!(io, "uname: {}", msg);
            ewln!(io, "Usage: uname [-asnrvmo]");
            return 1;
        }
    };
    if let Some(extra) = operands.first() {
        ewln!(io, "uname: extra operand '{}'", extra);
        return 1;
    }

    let wanted: Vec<char> = if flags.contains('a') {
        FIELDS.chars().collect()
    } else if flags.is_empty() {
        vec!['s']
    } else {
        FIELDS.chars().filter(|&f| flags.contains(f)).collect()
    };
    let values: Vec<String> = wanted.into_iter().map(field).collect();
    wln!(io, "{}", values.join(" "));
    0
}

/// The value of one field, or `unknown` when the system won't say.
///
/// - `s`: kernel name, as `uname -s` spells it on each platform
/// - `n`: network host name
/// - `r`: kernel release (the build number on Windows)
/// - `v`: operating system version
/// - `m`: machine hardware name
/// - `o`: operating system, by its full name
fn field(flag: char) -> String {
    let value = match flag {
        's' => Some(kernel_name().to_string()),
        'n' => System::host_name(),
        'r' => System::kernel_version(),
        'v' => System::os_version(),
        'm' => Some(System::cpu_arch()),
        'o' => System::long_os_version(),
        _ => unreachable!("uname: no field for -{}", flag),
    };
    value
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

/// The kernel's name. Not sysinfo's `System::name`, which on Linux is the
/// distribution ("Ubuntu") rather than what scripts that check
/// `uname -s` expect to see.
fn kernel_name() -> &'static str {
    match env::consts::OS {
        "linux" | "android" => "Linux",
        "macos" | "ios" => "Darwin",
        "windows" => "Windows_NT",
        "freebsd" => "FreeBSD",
        "netbsd" => "NetBSD",
        "openbsd" => "OpenBSD",
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::{field, kernel_name, FIELDS};
    use crate::test_support::run;

    #[test]
    fn prints_the_kernel_name_by_default() {
        let out = run("uname", &[], "");
        assert_eq!(out.status, 0, "{}", out.stderr);
        assert_eq!(out.stdout, format!("{}\n", kernel_name()));
    }

    #[test]
    fn names_the_kernel_the_way_scripts_expect() {
        let expected = if cfg!(target_os = "linux") {
            "Linux"
        } else if cfg!(target_os = "macos") {
            "Darwin"
        } else if cfg!(windows) {
            "Windows_NT"
        } else {
            return;
        };
        assert_eq!(kernel_name(), expected);
    }

    #[test]
    fn fields_come_out_in_a_fixed_order() {
        let expected = format!("{} {}\n", kernel_name(), field('n'));
        assert_eq!(run("uname", &["-ns"], "").stdout, expected);
        assert_eq!(run("uname", &["-n", "-s", "-n"], "").stdout, expected);
    }

    #[test]
    fn dash_a_prints_every_field() {
        let all: Vec<String> = FIELDS.chars().map(field).collect();
        assert_eq!(
            run("uname", &["-a"], "").stdout,
            format!("{}\n", all.join(" "))
        );
        assert!(all.iter().all(|value| !value.is_empty()));
    }

    #[test]
    fn takes_no_operands() {
        let out = run("uname", &["x"], "");
        assert_eq!(out.status, 1);
        assert!(out.stderr.contains("extra operand 'x'"));
    }
}
