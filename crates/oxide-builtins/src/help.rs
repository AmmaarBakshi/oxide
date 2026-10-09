use crate::{registry, Ctx, Io, EXECUTOR_BUILTINS};

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    // Detailed help for each command named. An unknown name is reported but
    // doesn't stop the rest, as with bash's `help cd pwd`.
    if !args.is_empty() {
        let mut status = 0;
        for name in args {
            match registry().get(name) {
                Some(builtin) => wln!(io, "{} — {}", builtin.name(), builtin.help()),
                None if EXECUTOR_BUILTINS.contains(&name.as_str()) => {
                    wln!(
                        io,
                        "{} — built in to the shell; no detailed help yet.",
                        name
                    )
                }
                None => {
                    ewln!(io, "oxide: help: no such builtin '{}'", name);
                    status = 1;
                }
            }
        }
        return status;
    }

    // The whole list, straight from the registry so it can never go stale.
    wln!(io, "Built-in commands:");
    for builtin in registry().iter() {
        wln!(io, "  {:<9} {}", builtin.name(), builtin.help());
    }

    wln!(io, "");
    wln!(io, "Also built in: {}", EXECUTOR_BUILTINS.join(", "));
    0
}

#[cfg(test)]
mod tests {
    use crate::test_support::run;

    #[test]
    fn describes_every_name_given() {
        let out = run("help", &["cd", "pwd"], "");
        assert_eq!(out.status, 0, "{}", out.stderr);
        assert_eq!(
            out.stdout,
            "cd — change the working directory
pwd — print the working directory
"
        );
    }

    #[test]
    fn an_unknown_name_fails_but_the_rest_are_still_shown() {
        let out = run("help", &["nope", "cd"], "");
        assert_eq!(out.status, 1);
        assert_eq!(
            out.stdout,
            "cd — change the working directory
"
        );
        assert_eq!(
            out.stderr,
            "oxide: help: no such builtin 'nope'
"
        );
    }
}
