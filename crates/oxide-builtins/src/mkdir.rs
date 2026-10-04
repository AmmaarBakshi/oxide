use std::fs;

use crate::{Ctx, Io};

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    if args.is_empty() {
        ewln!(io, "oxide: mkdir: missing operand (e.g., 'mkdir dir' or 'mkdir -p a/b/c')");
        return 1;
    }

    // `-p` creates parent directories as needed and is not an error if the
    // target already exists.
    let parents = args.iter().any(|a| a == "-p");
    let dirs: Vec<&String> = args.iter().filter(|a| !a.starts_with('-')).collect();

    if dirs.is_empty() {
        ewln!(io, "oxide: mkdir: missing operand");
        return 1;
    }

    let mut status = 0;
    for dir in dirs {
        let result = if parents {
            fs::create_dir_all(dir)
        } else {
            fs::create_dir(dir)
        };
        if let Err(e) = result {
            ewln!(io, "oxide: mkdir: cannot create directory '{}': {}", dir, e);
            status = 1;
        }
    }
    status
}
