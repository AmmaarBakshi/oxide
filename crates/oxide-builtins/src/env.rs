use crate::{Ctx, Io};

pub fn run(_args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    for (key, value) in std::env::vars() {
        wln!(io, "{}={}", key, value);
    }
    0
}
