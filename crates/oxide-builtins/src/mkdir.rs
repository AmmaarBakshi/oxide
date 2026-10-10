use std::fs;

use crate::args::parse_flags;
use crate::{Ctx, Io};

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    let (flags, dirs) = match parse_flags(args, "p") {
        Ok(parsed) => parsed,
        Err(msg) => {
            ewln!(io, "oxide: mkdir: {}", msg);
            ewln!(io, "Usage: mkdir [-p] <dir>...");
            return 1;
        }
    };

    if dirs.is_empty() {
        ewln!(io, "oxide: mkdir: missing operand (e.g., 'mkdir dir' or 'mkdir -p a/b/c')");
        return 1;
    }

    // `-p` creates parent directories as needed and is not an error if the
    // target already exists.
    let parents = flags.contains('p');

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

#[cfg(test)]
mod tests {
    use crate::test_support::{run, scratch_dir};

    #[test]
    fn dash_p_creates_parents() {
        let dir = scratch_dir("mkdir_parents");
        let nested = dir.join("a").join("b");
        let nested = nested.to_str().unwrap();

        assert_eq!(run("mkdir", &[nested], "").status, 1);
        let out = run("mkdir", &["-p", nested], "");
        assert_eq!(out.status, 0, "{}", out.stderr);
        assert!(dir.join("a").join("b").is_dir());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn rejects_unknown_flags() {
        let dir = scratch_dir("mkdir_flags");
        let target = dir.join("new");

        let out = run("mkdir", &["-x", target.to_str().unwrap()], "");
        assert_eq!(out.status, 1);
        assert!(out.stderr.contains("invalid option -- 'x'"), "{}", out.stderr);
        assert!(!target.exists());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
