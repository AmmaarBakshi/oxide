use crate::{registry, Ctx, Io, EXECUTOR_BUILTINS, REPL_BUILTINS};

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    // Detailed help for one command.
    if let Some(name) = args.first() {
        return match registry().get(name) {
            Some(builtin) => {
                wln!(io, "{} — {}", builtin.name(), builtin.help());
                0
            }
            None if EXECUTOR_BUILTINS.contains(&name.as_str())
                || REPL_BUILTINS.contains(&name.as_str()) =>
            {
                wln!(io, "{} — built in to the shell; no detailed help yet.", name);
                0
            }
            None => {
                ewln!(io, "oxide: help: no such builtin '{}'", name);
                1
            }
        };
    }

    // The whole list, straight from the registry so it can never go stale.
    wln!(io, "Built-in commands:");
    for builtin in registry().iter() {
        wln!(io, "  {:<9} {}", builtin.name(), builtin.help());
    }

    wln!(io, "");
    wln!(io, "Also built in: {}", EXECUTOR_BUILTINS.join(", "));
    wln!(io, "              {}", REPL_BUILTINS.join(", "));
    0
}
