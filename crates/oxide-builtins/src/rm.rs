use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use crate::args::parse_flags;
use crate::{Ctx, Io};

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    let (flags, paths) = match parse_flags(args, "rRf") {
        Ok(parsed) => parsed,
        Err(msg) => {
            ewln!(io, "oxide: rm: {}", msg);
            ewln!(io, "Usage: rm [-rf] <path>...");
            return 1;
        }
    };
    let recursive = flags.contains(['r', 'R']);
    let force = flags.contains('f');

    if paths.is_empty() {
        if force {
            return 0;
        }
        ewln!(
            io,
            "oxide: rm: missing operand (e.g., 'rm file.txt' or 'rm -r dir')"
        );
        return 1;
    }

    let mut status = 0;
    for arg in paths {
        let path = Path::new(arg);
        // `rm -r .` would empty the working directory before failing on the
        // directory itself, so refuse it up front as coreutils does.
        if is_dot_or_dotdot(path) {
            ewln!(
                io,
                "oxide: rm: refusing to remove '.' or '..' directory: skipping '{}'",
                arg
            );
            status = 1;
            continue;
        }
        let result = if path.is_dir() {
            // Deleting a whole tree has to be asked for explicitly.
            if !recursive {
                ewln!(
                    io,
                    "oxide: rm: cannot remove '{}': is a directory (use -r)",
                    arg
                );
                status = 1;
                continue;
            }
            fs::remove_dir_all(path)
        } else {
            fs::remove_file(path)
        };
        match result {
            Ok(()) => {}
            // With -f, a path that's already gone is the outcome we wanted.
            Err(e) if force && e.kind() == ErrorKind::NotFound => {}
            Err(e) => {
                ewln!(io, "oxide: rm: cannot remove '{}': {}", arg, e);
                status = 1;
            }
        }
    }
    status
}

/// Whether the last component of `path` is `.` or `..` (`.`, `../..`, `dir/.`).
/// `Path::components` normalizes a trailing `.` away, so the text is split by
/// hand instead.
fn is_dot_or_dotdot(path: &Path) -> bool {
    let text = path.to_string_lossy();
    let last = text
        .trim_end_matches(std::path::is_separator)
        .rsplit(std::path::is_separator)
        .next();
    matches!(last, Some("." | ".."))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::test_support::{run, scratch_dir};

    #[test]
    fn removes_files() {
        let dir = scratch_dir("rm_files");
        let file = dir.join("a.txt");
        fs::write(&file, "x").unwrap();

        assert_eq!(run("rm", &[file.to_str().unwrap()], "").status, 0);
        assert!(!file.exists());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn directories_need_the_recursive_flag() {
        let dir = scratch_dir("rm_recursive");
        let sub = dir.join("sub");
        fs::create_dir(&sub).unwrap();
        fs::write(sub.join("keep.txt"), "x").unwrap();

        let out = run("rm", &[sub.to_str().unwrap()], "");
        assert_eq!(out.status, 1);
        assert!(out.stderr.contains("is a directory"));
        assert!(sub.join("keep.txt").exists());

        assert_eq!(run("rm", &["-r", sub.to_str().unwrap()], "").status, 0);
        assert!(!sub.exists());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn refuses_dot_and_dotdot_but_removes_the_rest() {
        let dir = scratch_dir("rm_dot");
        let sub = dir.join("sub");
        fs::create_dir(&sub).unwrap();
        fs::write(sub.join("keep.txt"), "x").unwrap();
        let file = dir.join("a.txt");
        fs::write(&file, "x").unwrap();

        let dot = format!("{}/.", sub.to_str().unwrap());
        let dotdot = format!("{}/../", sub.to_str().unwrap());
        let out = run("rm", &["-rf", &dot, &dotdot, file.to_str().unwrap()], "");
        assert_eq!(out.status, 1);
        assert_eq!(
            out.stderr.matches("refusing to remove").count(),
            2,
            "{}",
            out.stderr
        );
        assert!(sub.join("keep.txt").exists());
        assert!(!file.exists());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn force_ignores_missing_paths() {
        let dir = scratch_dir("rm_force");
        let missing = dir.join("missing.txt");

        assert_eq!(run("rm", &[missing.to_str().unwrap()], "").status, 1);
        let out = run("rm", &["-f", missing.to_str().unwrap()], "");
        assert_eq!((out.status, out.stderr.as_str()), (0, ""));
        fs::remove_dir_all(dir).unwrap();
    }
}
