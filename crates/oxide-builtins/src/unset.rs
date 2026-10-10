use std::env;

use crate::export::is_valid_name;
use crate::{Ctx, Io};

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    if args.is_empty() {
        ewln!(io, "oxide: unset: missing variable name");
        return 1;
    }

    let mut status = 0;
    for var in args {
        // `remove_var` panics on an empty name or one containing '=' or NUL.
        if is_valid_name(var) {
            env::remove_var(var);
        } else {
            ewln!(io, "oxide: unset: '{}': not a valid identifier", var);
            status = 1;
        }
    }
    status
}

#[cfg(test)]
mod tests {
    use crate::test_support::run;

    #[test]
    fn removes_each_variable() {
        std::env::set_var("OXIDE_TEST_UNSET_A", "1");
        std::env::set_var("OXIDE_TEST_UNSET_B", "2");
        let out = run("unset", &["OXIDE_TEST_UNSET_A", "OXIDE_TEST_UNSET_B"], "");
        assert_eq!(out.status, 0, "{}", out.stderr);
        assert!(std::env::var_os("OXIDE_TEST_UNSET_A").is_none());
        assert!(std::env::var_os("OXIDE_TEST_UNSET_B").is_none());
    }

    #[test]
    fn rejects_invalid_names_instead_of_panicking() {
        std::env::set_var("OXIDE_TEST_UNSET_C", "3");
        let out = run("unset", &["", "A=B", "OXIDE_TEST_UNSET_C"], "");
        assert_eq!(out.status, 1);
        assert_eq!(out.stderr.matches("not a valid identifier").count(), 2, "{}", out.stderr);
        assert!(std::env::var_os("OXIDE_TEST_UNSET_C").is_none());
    }
}
