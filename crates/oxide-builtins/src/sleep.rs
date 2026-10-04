use std::thread;
use std::time::Duration;

use crate::{Ctx, Io};

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    if args.is_empty() {
        ewln!(io, "oxide: sleep: missing operand (e.g., 'sleep 5')");
        return 1;
    }

    if let Ok(secs) = args[0].parse::<u64>() {
        thread::sleep(Duration::from_secs(secs));
        0
    } else {
        ewln!(io, "oxide: sleep: invalid time interval '{}'", args[0]);
        1
    }
}
