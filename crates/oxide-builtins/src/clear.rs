use crate::{Ctx, Io};

pub fn run(_args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    // Clear screen + clear scrollback/history + move cursor to top-left
    if io.stdout.write_all(b"\x1B[3J\x1B[2J\x1B[H").is_err() {
        return 1;
    }
    if io.stdout.flush().is_err() {
        return 1;
    }
    0
}
