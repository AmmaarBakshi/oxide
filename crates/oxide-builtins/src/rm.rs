use std::fs;
use std::path::Path;

use crate::{Ctx, Io};

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    if args.is_empty() {
        ewln!(io, "oxide: rm: missing operand (e.g., 'rm file.txt')");
        return 1;
    }

    let mut status = 0;
    for arg in args {
        let path = Path::new(arg);
        let result = if path.is_dir() {
            // Delete folder and everything inside it
            fs::remove_dir_all(path)
        } else {
            fs::remove_file(path)
        };
        if let Err(e) = result {
            ewln!(io, "oxide: rm: cannot remove '{}': {}", arg, e);
            status = 1;
        }
    }
    status
}
