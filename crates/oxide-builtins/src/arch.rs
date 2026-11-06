use crate::args::parse_flags;
use crate::{Ctx, Io};

/// `arch` is `uname -m` under another name.
pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    let operands = match parse_flags(args, "") {
        Ok((_, operands)) => operands,
        Err(msg) => {
            ewln!(io, "arch: {}", msg);
            ewln!(io, "Usage: arch");
            return 1;
        }
    };
    if let Some(extra) = operands.first() {
        ewln!(io, "arch: extra operand '{}'", extra);
        return 1;
    }
    wln!(io, "{}", crate::uname::field('m'));
    0
}

#[cfg(test)]
mod tests {
    use crate::test_support::run;

    #[test]
    fn prints_what_uname_dash_m_does() {
        let out = run("arch", &[], "");
        assert_eq!(out.status, 0, "{}", out.stderr);
        assert_eq!(out.stdout, run("uname", &["-m"], "").stdout);
        assert_ne!(out.stdout.trim(), "");
    }

    #[test]
    fn takes_no_operands_or_flags() {
        let out = run("arch", &["x"], "");
        assert_eq!(out.status, 1);
        assert!(out.stderr.contains("extra operand 'x'"));
        assert_eq!(run("arch", &["-m"], "").status, 1);
    }
}
