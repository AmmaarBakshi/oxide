use std::env;

use oxide_parser::arith;

use crate::{Ctx, Io};

pub fn run(args: &[String], io: &mut Io<'_>, ctx: &mut Ctx<'_>) -> i32 {
    if args.is_empty() {
        ewln!(io, "oxide: let: expression expected");
        return 1;
    }
    let mut last = 0;
    for expr in args {
        match arith::evaluate(expr, &mut ShellVars(ctx)) {
            Ok(value) => last = value,
            Err(e) => {
                ewln!(io, "oxide: let: {}: {}", expr, e);
                return 1;
            }
        }
    }
    // Like `(( ))` elsewhere, a final value of 0 counts as false.
    if last == 0 {
        1
    } else {
        0
    }
}

/// The shell's variables, as arithmetic sees them: shell variables first,
/// then the environment.
struct ShellVars<'c, 'a>(&'c mut Ctx<'a>);

impl arith::Vars for ShellVars<'_, '_> {
    fn get(&self, name: &str) -> Option<String> {
        self.0
            .vars
            .get(name)
            .cloned()
            .or_else(|| env::var(name).ok())
    }

    fn set(&mut self, name: &str, value: i64) {
        if !self.0.vars.contains_key(name) && env::var_os(name).is_some() {
            env::set_var(name, value.to_string());
        } else {
            self.0.vars.insert(name.to_string(), value.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::test_support::{run, run_with_vars};

    #[test]
    fn each_expression_is_evaluated_in_turn() {
        let (out, vars) = run_with_vars("let", &["x = 2 + 3", "y=x*2", "y++"], "");
        assert_eq!(out.status, 0, "{}", out.stderr);
        assert_eq!(vars.get("x").map(String::as_str), Some("5"));
        assert_eq!(vars.get("y").map(String::as_str), Some("11"));
    }

    #[test]
    fn the_status_says_whether_the_last_value_is_nonzero() {
        assert_eq!(run("let", &["3 > 2"], "").status, 0);
        assert_eq!(run("let", &["3 < 2"], "").status, 1);
        assert_eq!(run("let", &["5", "0"], "").status, 1);
    }

    #[test]
    fn errors_are_reported() {
        let out = run("let", &["1 / 0"], "");
        assert_eq!(out.status, 1);
        assert_eq!(out.stderr, "oxide: let: 1 / 0: division by zero\n");
        assert_eq!(run("let", &[], "").status, 1);
    }
}
