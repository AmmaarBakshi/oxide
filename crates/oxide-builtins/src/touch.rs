use std::fs::OpenOptions;

use crate::{Ctx, Io};

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    if args.is_empty() {
        ewln!(io, "touch: missing file operand");
        return 1;
    }
    for path in args {
        if let Err(e) = OpenOptions::new().create(true).write(true).open(path) {
            ewln!(io, "touch: cannot touch '{}': {}", path, e);
            return 1;
        }
    }
    0
}
