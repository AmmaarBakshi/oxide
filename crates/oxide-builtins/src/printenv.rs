use std::env;

use crate::args::parse_flags;
use crate::{Ctx, Io};

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    let (_, names) = match parse_flags(args, "") {
        Ok(parsed) => parsed,
        Err(msg) => {
            ewln!(io, "printenv: {}", msg);
            ewln!(io, "Usage: printenv [NAME...]");
            return 1;
        }
    };

    if names.is_empty() {
        for (key, value) in env::vars() {
            wln!(io, "{}={}", key, value);
        }
        return 0;
    }

    // Only exported variables are printed, as a child process would see
    // them. A name that isn't set prints nothing but fails the command.
    let mut status = 0;
    for name in names {
        match env::var(name) {
            Ok(value) => wln!(io, "{}", value),
            Err(_) => status = 1,
        }
    }
    status
}

#[cfg(test)]
mod tests {
    use crate::test_support::run;

    const UNSET: &str = "OXIDE_PRINTENV_TEST_UNSET";

    #[test]
    fn prints_the_values_of_named_variables() {
        let path = std::env::var("PATH").expect("PATH is set");
        let out = run("printenv", &["PATH"], "");
        assert_eq!((out.status, out.stdout), (0, format!("{}\n", path)));
    }

    #[test]
    fn a_missing_name_fails_without_output() {
        let path = std::env::var("PATH").expect("PATH is set");
        let out = run("printenv", &[UNSET, "PATH"], "");
        assert_eq!((out.status, out.stdout), (1, format!("{}\n", path)));
        assert_eq!(out.stderr, "");
    }

    #[test]
    fn no_names_prints_the_whole_environment() {
        let out = run("printenv", &[], "");
        assert_eq!(out.status, 0);
        assert!(out
            .stdout
            .lines()
            .any(|line| line.to_ascii_uppercase().starts_with("PATH=")));
    }
}
