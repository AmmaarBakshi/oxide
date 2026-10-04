use std::env;

use crate::{Ctx, Io};

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    if args.is_empty() {
        ewln!(io, "oxide: export: missing argument");
        return 1;
    }

    for arg in args {
        // Split the argument by the '=' sign (e.g., "NAME=Atlas")
        if let Some((key, value)) = arg.split_once('=') {
            env::set_var(key, value);
        } else {
            ewln!(io, "oxide: export: invalid format '{}'. Use KEY=VALUE", arg);
            return 1;
        }
    }
    0
}
