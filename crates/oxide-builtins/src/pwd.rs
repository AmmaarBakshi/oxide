use std::env;

use crate::{Ctx, Io};

pub fn run(_args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    match env::current_dir() {
        Ok(dir) => {
            wln!(io, "{}", dir.display());
            0
        }
        Err(e) => {
            ewln!(io, "oxide: pwd: {}", e);
            1
        }
    }
}
