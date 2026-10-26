use std::fs;
use std::path::Path;

use crate::args::parse_flags;
use crate::{Ctx, Io};

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    let (flags, dirs) = match parse_flags(args, "p") {
        Ok(parsed) => parsed,
        Err(msg) => {
            ewln!(io, "oxide: rmdir: {}", msg);
            ewln!(io, "Usage: rmdir [-p] <dir>...");
            return 1;
        }
    };

    if dirs.is_empty() {
        ewln!(io, "oxide: rmdir: missing operand");
        return 1;
    }

    // `-p` removes each named directory and then its parents in turn, so
    // `rmdir -p a/b/c` undoes `mkdir -p a/b/c`.
    let parents = flags.contains('p');

    let mut status = 0;
    for dir in dirs {
        let mut target = Path::new(dir);
        loop {
            // Only ever removes empty directories: unlike `rm -r`, `rmdir`
            // can't lose data.
            if let Err(e) = fs::remove_dir(target) {
                ewln!(
                    io,
                    "oxide: rmdir: failed to remove '{}': {}",
                    target.display(),
                    e
                );
                status = 1;
                break;
            }
            match target.parent() {
                Some(parent) if parents && !parent.as_os_str().is_empty() => target = parent,
                _ => break,
            }
        }
    }
    status
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::test_support::{run, scratch_dir};

    #[test]
    fn removes_empty_directories() {
        let dir = scratch_dir("rmdir_empty");
        let empty = dir.join("empty");
        fs::create_dir(&empty).unwrap();

        let out = run("rmdir", &[empty.to_str().unwrap()], "");
        assert_eq!(out.status, 0, "{}", out.stderr);
        assert!(!empty.exists());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn refuses_a_directory_with_contents() {
        let dir = scratch_dir("rmdir_full");
        fs::write(dir.join("keep.txt"), "x").unwrap();

        let out = run("rmdir", &[dir.to_str().unwrap()], "");
        assert_eq!(out.status, 1);
        assert!(out.stderr.contains("failed to remove"), "{}", out.stderr);
        assert!(dir.join("keep.txt").exists());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn dash_p_removes_parents_until_one_is_not_empty() {
        let dir = scratch_dir("rmdir_parents");
        fs::create_dir_all(dir.join("a").join("b")).unwrap();
        fs::write(dir.join("stay.txt"), "x").unwrap();

        let nested = dir.join("a").join("b");
        let out = run("rmdir", &["-p", nested.to_str().unwrap()], "");
        assert!(!dir.join("a").exists());
        // `dir` itself still holds stay.txt, which ends the walk with an error.
        assert_eq!(out.status, 1);
        assert!(dir.join("stay.txt").exists());
        fs::remove_dir_all(dir).unwrap();
    }
}
