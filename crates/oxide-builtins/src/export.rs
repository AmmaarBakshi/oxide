use std::env;

use crate::{Ctx, Io};

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    if args.is_empty() {
        ewln!(io, "oxide: export: missing argument");
        return 1;
    }

    let mut status = 0;
    for arg in args {
        // Split the argument by the '=' sign (e.g., "NAME=Atlas")
        let Some((key, value)) = arg.split_once('=') else {
            ewln!(io, "oxide: export: invalid format '{}'. Use KEY=VALUE", arg);
            status = 1;
            continue;
        };
        // `set_var` panics on an empty name or a NUL byte, so check first.
        if !is_valid_name(key) {
            ewln!(io, "oxide: export: '{}': not a valid identifier", key);
            status = 1;
        } else if value.contains('\0') {
            ewln!(io, "oxide: export: '{}': value contains a NUL byte", key);
            status = 1;
        } else {
            env::set_var(key, value);
        }
    }
    status
}

/// Whether `name` can be a variable name: a letter or underscore, then
/// letters, digits and underscores — the same rule bash applies.
pub(crate) fn is_valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[cfg(test)]
mod tests {
    use crate::test_support::run;

    #[test]
    fn sets_each_variable() {
        let out = run("export", &["OXIDE_TEST_EXPORT_A=1", "OXIDE_TEST_EXPORT_B=two words"], "");
        assert_eq!(out.status, 0, "{}", out.stderr);
        assert_eq!(std::env::var("OXIDE_TEST_EXPORT_A").unwrap(), "1");
        assert_eq!(std::env::var("OXIDE_TEST_EXPORT_B").unwrap(), "two words");
    }

    #[test]
    fn rejects_invalid_names_instead_of_panicking() {
        for bad in ["=x", "1A=x", "A-B=x", "A\0B=x"] {
            let out = run("export", &[bad], "");
            assert_eq!(out.status, 1, "{bad:?} was accepted");
            assert!(out.stderr.contains("not a valid identifier"), "{}", out.stderr);
        }
        let out = run("export", &["OXIDE_TEST_EXPORT_NUL=a\0b"], "");
        assert_eq!(out.status, 1);
        assert!(std::env::var_os("OXIDE_TEST_EXPORT_NUL").is_none());
    }

    #[test]
    fn a_bad_argument_does_not_stop_the_rest() {
        let out = run("export", &["=x", "OXIDE_TEST_EXPORT_C=ok"], "");
        assert_eq!(out.status, 1);
        assert_eq!(std::env::var("OXIDE_TEST_EXPORT_C").unwrap(), "ok");
    }
}
