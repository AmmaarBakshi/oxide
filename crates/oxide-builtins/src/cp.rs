use std::fs;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};

use crate::args::parse_flags;
use crate::{Ctx, Io};

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    let (flags, operands) = match parse_flags(args, "rR") {
        Ok(parsed) => parsed,
        Err(msg) => {
            ewln!(io, "cp: {}", msg);
            ewln!(io, "Usage: cp [-r] <source>... <dest>");
            return 1;
        }
    };
    let recursive = flags.contains(['r', 'R']);

    let Some((dest, sources)) = operands.split_last().filter(|(_, s)| !s.is_empty()) else {
        ewln!(io, "cp: missing destination operand (e.g., 'cp a.txt b.txt' or 'cp -r src dir')");
        return 1;
    };

    let pairs = match destinations(sources, Path::new(dest)) {
        Ok(pairs) => pairs,
        Err(msg) => {
            ewln!(io, "cp: {}", msg);
            return 1;
        }
    };

    let mut status = 0;
    for (src, target) in pairs {
        if let Err(msg) = copy_path(src, &target, recursive) {
            ewln!(io, "cp: {}", msg);
            status = 1;
        }
    }
    status
}

/// Pairs each source with where it should end up: inside `dest` when that is
/// an existing directory, otherwise at `dest` itself, which then admits only
/// one source. Shared with `mv`.
pub(crate) fn destinations<'a>(
    sources: &[&'a str],
    dest: &Path,
) -> Result<Vec<(&'a Path, PathBuf)>, String> {
    if dest.is_dir() {
        sources
            .iter()
            .map(|src| {
                let src = Path::new(*src);
                match src.file_name() {
                    Some(name) => Ok((src, dest.join(name))),
                    None => Err(format!("cannot use '{}' as a source", src.display())),
                }
            })
            .collect()
    } else if let [src] = sources {
        Ok(vec![(Path::new(*src), dest.to_path_buf())])
    } else {
        Err(format!("target '{}' is not a directory", dest.display()))
    }
}

/// Copies `src` to `dst`, descending into directories when `recursive`, and
/// describes the failure if it can't. Shared with `mv`, which falls back to
/// copy-then-delete when a rename can't cross filesystems.
pub(crate) fn copy_path(src: &Path, dst: &Path, recursive: bool) -> Result<(), String> {
    let meta = fs::metadata(src).map_err(|e| format!("cannot stat '{}': {}", src.display(), e))?;
    let failed =
        |e: io::Error| format!("cannot copy '{}' to '{}': {}", src.display(), dst.display(), e);

    if meta.is_dir() {
        if !recursive {
            return Err(format!("-r not specified; omitting directory '{}'", src.display()));
        }
        // Otherwise the walk would keep finding the copy it is making.
        if is_within(dst, src) {
            return Err(format!(
                "cannot copy a directory, '{}', into itself, '{}'",
                src.display(),
                dst.display()
            ));
        }
        return copy_dir(src, dst).map_err(failed);
    }

    // `fs::copy` truncates the destination before reading the source, so
    // copying a file onto itself would empty it.
    if same_file(src, dst) {
        return Err(format!("'{}' and '{}' are the same file", src.display(), dst.display()));
    }
    fs::copy(src, dst).map(drop).map_err(failed)
}

/// Copies the tree at `src` to `dst`, merging into `dst` if it already exists.
///
/// Uses `create_dir` rather than `create_dir_all`: requiring the parent to
/// exist is what lets `is_within` resolve `dst` before the walk starts.
fn copy_dir(src: &Path, dst: &Path) -> io::Result<()> {
    match fs::create_dir(dst) {
        Ok(()) => {}
        Err(e) if e.kind() == ErrorKind::AlreadyExists && dst.is_dir() => {}
        Err(e) => return Err(e),
    }
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let target = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

/// Whether `path` is `dir` itself or lies somewhere beneath it. `path` need
/// not exist yet, but its parent must.
pub(crate) fn is_within(path: &Path, dir: &Path) -> bool {
    let Ok(dir) = dir.canonicalize() else {
        return false;
    };
    resolve(path).is_some_and(|path| path.starts_with(dir))
}

/// Canonicalizes `path`, or for a path that doesn't exist yet, its parent
/// joined with the final component.
fn resolve(path: &Path) -> Option<PathBuf> {
    if let Ok(path) = path.canonicalize() {
        return Some(path);
    }
    let parent = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    Some(parent.canonicalize().ok()?.join(path.file_name()?))
}

fn same_file(a: &Path, b: &Path) -> bool {
    matches!((a.canonicalize(), b.canonicalize()), (Ok(a), Ok(b)) if a == b)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::test_support::{run, scratch_dir};

    #[test]
    fn copies_a_file() {
        let dir = scratch_dir("cp_file");
        let (a, b) = (dir.join("a.txt"), dir.join("b.txt"));
        fs::write(&a, "hello").unwrap();

        let out = run("cp", &[a.to_str().unwrap(), b.to_str().unwrap()], "");
        assert_eq!(out.status, 0, "{}", out.stderr);
        assert_eq!(fs::read_to_string(&b).unwrap(), "hello");
        assert!(a.exists());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn copies_files_into_a_directory() {
        let dir = scratch_dir("cp_into_dir");
        let (a, b, dest) = (dir.join("a.txt"), dir.join("b.txt"), dir.join("dest"));
        fs::write(&a, "a").unwrap();
        fs::write(&b, "b").unwrap();
        fs::create_dir(&dest).unwrap();

        let args = [a.to_str().unwrap(), b.to_str().unwrap(), dest.to_str().unwrap()];
        let out = run("cp", &args, "");
        assert_eq!(out.status, 0, "{}", out.stderr);
        assert_eq!(fs::read_to_string(dest.join("a.txt")).unwrap(), "a");
        assert_eq!(fs::read_to_string(dest.join("b.txt")).unwrap(), "b");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn several_sources_need_a_directory_target() {
        let dir = scratch_dir("cp_not_dir");
        let a = dir.join("a.txt");
        fs::write(&a, "a").unwrap();
        let missing = dir.join("missing");

        let args = [a.to_str().unwrap(), a.to_str().unwrap(), missing.to_str().unwrap()];
        let out = run("cp", &args, "");
        assert_eq!(out.status, 1);
        assert!(out.stderr.contains("is not a directory"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn directories_need_the_recursive_flag() {
        let dir = scratch_dir("cp_recursive");
        let (src, dst) = (dir.join("src"), dir.join("dst"));
        fs::create_dir_all(src.join("nested")).unwrap();
        fs::write(src.join("nested").join("f.txt"), "deep").unwrap();
        let (src_arg, dst_arg) = (src.to_str().unwrap(), dst.to_str().unwrap());

        let out = run("cp", &[src_arg, dst_arg], "");
        assert_eq!(out.status, 1);
        assert!(out.stderr.contains("-r not specified"));

        let out = run("cp", &["-r", src_arg, dst_arg], "");
        assert_eq!(out.status, 0, "{}", out.stderr);
        assert_eq!(fs::read_to_string(dst.join("nested").join("f.txt")).unwrap(), "deep");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn refuses_to_copy_a_directory_into_itself() {
        let dir = scratch_dir("cp_into_itself");
        let src = dir.join("src");
        fs::create_dir(&src).unwrap();
        fs::write(src.join("f.txt"), "x").unwrap();
        let inside = src.join("sub");

        let out = run("cp", &["-r", src.to_str().unwrap(), inside.to_str().unwrap()], "");
        assert_eq!(out.status, 1);
        assert!(out.stderr.contains("into itself"));
        assert!(!inside.exists());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn refuses_to_copy_a_file_onto_itself() {
        let dir = scratch_dir("cp_same_file");
        let a = dir.join("a.txt");
        fs::write(&a, "keep me").unwrap();

        let out = run("cp", &[a.to_str().unwrap(), a.to_str().unwrap()], "");
        assert_eq!(out.status, 1);
        assert!(out.stderr.contains("are the same file"));
        assert_eq!(fs::read_to_string(&a).unwrap(), "keep me");
        fs::remove_dir_all(dir).unwrap();
    }
}
