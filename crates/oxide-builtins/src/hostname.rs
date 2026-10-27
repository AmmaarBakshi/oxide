use sysinfo::System;

use crate::args::parse_flags;
use crate::{Ctx, Io};

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    let (flags, operands) = match parse_flags(args, "s") {
        Ok(parsed) => parsed,
        Err(msg) => {
            ewln!(io, "hostname: {}", msg);
            ewln!(io, "Usage: hostname [-s]");
            return 1;
        }
    };
    // Setting the host name needs privileges a shell builtin shouldn't use.
    if let Some(extra) = operands.first() {
        ewln!(io, "oxide: hostname: extra operand '{}'", extra);
        return 1;
    }
    match System::host_name().filter(|name| !name.is_empty()) {
        Some(name) => {
            wln!(io, "{}", display_name(&name, flags.contains('s')));
            0
        }
        None => {
            ewln!(io, "oxide: hostname: cannot find the host name");
            1
        }
    }
}

/// The host name as printed: with `short`, cut at the first dot so
/// `build01.example.com` becomes `build01`.
fn display_name(name: &str, short: bool) -> &str {
    if short {
        name.split('.').next().unwrap_or(name)
    } else {
        name
    }
}

#[cfg(test)]
mod tests {
    use crate::test_support::run;

    #[test]
    fn prints_the_host_name() {
        let out = run("hostname", &[], "");
        assert_eq!(out.status, 0, "{}", out.stderr);
        assert!(!out.stdout.trim().is_empty());
    }

    #[test]
    fn dash_s_drops_the_domain() {
        assert_eq!(super::display_name("build01.example.com", true), "build01");
        assert_eq!(
            super::display_name("build01.example.com", false),
            "build01.example.com"
        );
        assert!(!run("hostname", &["-s"], "").stdout.contains('.'));
    }

    #[test]
    fn takes_no_operands() {
        let out = run("hostname", &["newname"], "");
        assert_eq!(out.status, 1);
        assert!(out.stderr.contains("extra operand 'newname'"));
    }
}
