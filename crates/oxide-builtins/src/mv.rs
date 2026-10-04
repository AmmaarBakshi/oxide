use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use crate::args::parse_flags;
use crate::cp::{copy_path, destinations, is_within};
use crate::{Ctx, Io};

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    // mv takes no flags yet, but parsing them still honors `--` and reports a
    // mistyped option instead of treating it as a file name.
    let (_, operands) = match parse_flags(args, "") {
        Ok(parsed) => parsed,
        Err(msg) => {
            ewln!(io, "mv: {}", msg);
            ewln!(io, "Usage: mv <source>... <dest>");
            return 1;
        }
    };

    let Some((dest, sources)) = operands.split_last().filter(|(_, s)| !s.is_empty()) else {
        ewln!(io, "mv: missing destination operand (e.g., 'mv old.txt new.txt' or 'mv a b dir')");
        return 1;
    };

    let pairs = match destinations(sources, Path::new(dest)) {
        Ok(pairs) => pairs,
        Err(msg) => {
            ewln!(io, "mv: {}", msg);
            return 1;
        }
    };

    let mut status = 0;
    for (src, target) in pairs {
        if let Err(msg) = move_path(src, &target) {
            ewln!(io, "mv: {}", msg);
            status = 1;
        }
    }
    status
}

fn move_path(src: &Path, dst: &Path) -> Result<(), String> {
    if src.is_dir() && is_within(dst, src) {
        return Err(format!(
            "cannot move '{}' to a subdirectory of itself, '{}'",
            src.display(),
            dst.display()
        ));
    }

    match fs::rename(src, dst) {
        Ok(()) => Ok(()),
        // A rename can't cross filesystems (or drives, on Windows).
        Err(e) if e.kind() == ErrorKind::CrossesDevices => copy_then_remove(src, dst),
        Err(e) => Err(format!("cannot move '{}' to '{}': {}", src.display(), dst.display(), e)),
    }
}

/// The slow path for a move a rename can't do: copy everything, and only
/// once that has fully succeeded, delete the original.
fn copy_then_remove(src: &Path, dst: &Path) -> Result<(), String> {
    copy_path(src, dst, true)?;
    let removed = if src.is_dir() {
        fs::remove_dir_all(src)
    } else {
        fs::remove_file(src)
    };
    removed.map_err(|e| format!("copied '{}' but could not remove it: {}", src.display(), e))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::copy_then_remove;
    use crate::test_support::{run, scratch_dir};

    #[test]
    fn renames_a_file() {
        let dir = scratch_dir("mv_rename");
        let (a, b) = (dir.join("a.txt"), dir.join("b.txt"));
        fs::write(&a, "hello").unwrap();

        let out = run("mv", &[a.to_str().unwrap(), b.to_str().unwrap()], "");
        assert_eq!(out.status, 0, "{}", out.stderr);
        assert!(!a.exists());
        assert_eq!(fs::read_to_string(&b).unwrap(), "hello");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn moves_files_and_directories_into_a_directory() {
        let dir = scratch_dir("mv_into_dir");
        let (file, sub, dest) = (dir.join("a.txt"), dir.join("sub"), dir.join("dest"));
        fs::write(&file, "a").unwrap();
        fs::create_dir(&sub).unwrap();
        fs::write(sub.join("inner.txt"), "inner").unwrap();
        fs::create_dir(&dest).unwrap();

        let args = [file.to_str().unwrap(), sub.to_str().unwrap(), dest.to_str().unwrap()];
        let out = run("mv", &args, "");
        assert_eq!(out.status, 0, "{}", out.stderr);
        assert!(!file.exists() && !sub.exists());
        assert_eq!(fs::read_to_string(dest.join("a.txt")).unwrap(), "a");
        assert_eq!(fs::read_to_string(dest.join("sub").join("inner.txt")).unwrap(), "inner");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn refuses_to_move_a_directory_into_itself() {
        let dir = scratch_dir("mv_into_itself");
        let src = dir.join("src");
        fs::create_dir(&src).unwrap();

        let out = run("mv", &[src.to_str().unwrap(), src.join("sub").to_str().unwrap()], "");
        assert_eq!(out.status, 1);
        assert!(out.stderr.contains("subdirectory of itself"));
        assert!(src.is_dir());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn missing_source_is_reported() {
        let dir = scratch_dir("mv_missing");
        let (missing, dest) = (dir.join("missing.txt"), dir.join("dest.txt"));

        let out = run("mv", &[missing.to_str().unwrap(), dest.to_str().unwrap()], "");
        assert_eq!(out.status, 1);
        assert!(out.stderr.starts_with("mv: cannot move"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn cross_device_fallback_moves_a_whole_tree() {
        let dir = scratch_dir("mv_fallback");
        let (src, dst) = (dir.join("src"), dir.join("dst"));
        fs::create_dir_all(src.join("nested")).unwrap();
        fs::write(src.join("nested").join("f.txt"), "deep").unwrap();

        copy_then_remove(&src, &dst).unwrap();
        assert!(!src.exists());
        assert_eq!(fs::read_to_string(dst.join("nested").join("f.txt")).unwrap(), "deep");
        fs::remove_dir_all(dir).unwrap();
    }
}
