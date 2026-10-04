use std::env;

use crate::{Ctx, Io};

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    if args.is_empty() {
        ewln!(io, "oxide: unset: missing variable name");
        return 1;
    }

    for var in args {
        env::remove_var(var);
    }
    0
}
