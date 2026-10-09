//! `true`, `false` and `:`, which do nothing but set the exit status. They
//! matter most as conditions: `while true { ... }`.

use crate::{Ctx, Io};

pub fn run_true(_args: &[String], _io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    0
}

pub fn run_false(_args: &[String], _io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    1
}

#[cfg(test)]
mod tests {
    use crate::test_support::run;

    #[test]
    fn they_only_set_the_status() {
        for (name, status) in [("true", 0), (":", 0), ("false", 1)] {
            let out = run(name, &["ignored", "args"], "");
            assert_eq!(
                (out.status, out.stdout.as_str(), out.stderr.as_str()),
                (status, "", "")
            );
        }
    }
}
