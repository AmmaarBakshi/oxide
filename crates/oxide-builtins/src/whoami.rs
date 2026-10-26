use std::env;

use crate::{Ctx, Io};

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    if let Some(extra) = args.first() {
        ewln!(io, "oxide: whoami: extra operand '{}'", extra);
        return 1;
    }
    match current_user() {
        Some(user) => {
            wln!(io, "{}", user);
            0
        }
        None => {
            ewln!(io, "oxide: whoami: cannot find the current user name");
            1
        }
    }
}

/// The login name, from the variable each platform sets for it: `USERNAME`
/// on Windows, `USER` (or the older `LOGNAME`) elsewhere.
fn current_user() -> Option<String> {
    const VARS: &[&str] = if cfg!(windows) {
        &["USERNAME", "USER"]
    } else {
        &["USER", "LOGNAME"]
    };
    VARS.iter()
        .filter_map(|var| env::var(var).ok())
        .find(|name| !name.is_empty())
}

#[cfg(test)]
mod tests {
    use crate::test_support::run;

    #[test]
    fn prints_the_current_user() {
        let out = run("whoami", &[], "");
        match super::current_user() {
            Some(user) => assert_eq!((out.status, out.stdout), (0, format!("{}\n", user))),
            None => assert_eq!(out.status, 1),
        }
    }

    #[test]
    fn takes_no_operands() {
        let out = run("whoami", &["root"], "");
        assert_eq!(out.status, 1);
        assert!(out.stderr.contains("extra operand 'root'"));
    }
}
