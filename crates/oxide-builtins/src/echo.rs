use crate::{Ctx, Io};

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    // Redirection (`> file`, `>> file`) is applied by the caller when it
    // builds `io`, so echo only ever has to write one line.
    wln!(io, "{}", args.join(" "));
    0
}
