use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::args::parse_flags;
use crate::{Ctx, Io};

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    let (flags, operands) = match parse_flags(args, "fs") {
        Ok(parsed) => parsed,
        Err(msg) => {
            ewln!(io, "ln: {}", msg);
            usage(io);
            return 1;
        }
    };
    let symbolic = flags.contains('s');
    let force = flags.contains('f');

    // Either `ln TARGET LINK`, or every target linked by its own name into
    // a directory: the last operand, or the current one if there's just one.
    let (targets, dir) = match operands[..] {
        [] => {
            ewln!(io, "ln: missing file operand");
            usage(io);
            return 1;
        }
        [_] => (&operands[..], None),
        [target, link] if !Path::new(link).is_dir() => {
            return make_link(io, target, Path::new(link), symbolic, force);
        }
        [ref targets @ .., dir] => {
            if !Path::new(dir).is_dir() {
                ewln!(io, "ln: target '{}' is not a directory", dir);
                return 1;
            }
            (targets, Some(dir))
        }
    };

    let mut status = 0;
    for &target in targets {
        let Some(name) = Path::new(target).file_name() else {
            ewln!(io, "ln: cannot link to '{}': it has no file name", target);
            status = 1;
            continue;
        };
        let link = match dir {
            Some(dir) => Path::new(dir).join(name),
            None => PathBuf::from(name),
        };
        status |= make_link(io, target, &link, symbolic, force);
    }
    status
}

fn usage(io: &mut Io<'_>) {
    ewln!(io, "Usage: ln [-fs] TARGET [LINK]");
    ewln!(io, "       ln [-fs] TARGET... DIRECTORY");
}

/// Links `link` to `target`, first removing whatever is at `link` if
/// `force` is set.
fn make_link(io: &mut Io<'_>, target: &str, link: &Path, symbolic: bool, force: bool) -> i32 {
    if force {
        if let Ok(meta) = fs::symlink_metadata(link) {
            if meta.is_dir() {
                ewln!(io, "ln: '{}': cannot overwrite directory", link.display());
                return 1;
            }
            // Replacing a file with a link to itself would just delete it.
            if same_file(&resolve(target, link, symbolic), link) {
                ewln!(
                    io,
                    "ln: '{}' and '{}' are the same file",
                    target,
                    link.display()
                );
                return 1;
            }
            if let Err(e) = remove_link(link, &meta) {
                ewln!(io, "ln: cannot remove '{}': {}", link.display(), e);
                return 1;
            }
        }
    }

    let (result, kind) = if symbolic {
        (symlink(Path::new(target), link), "symbolic link")
    } else {
        (fs::hard_link(target, link), "hard link")
    };
    match result {
        Ok(()) => 0,
        Err(e) => {
            ewln!(
                io,
                "ln: failed to create {} '{}': {}",
                kind,
                link.display(),
                e
            );
            1
        }
    }
}

/// Where `target` actually points from `link`. A hard link's target is
/// looked up from the current directory, but a symlink's is resolved from
/// the directory the link lives in.
fn resolve(target: &str, link: &Path, symbolic: bool) -> PathBuf {
    match link.parent() {
        Some(base) if symbolic => base.join(target),
        _ => PathBuf::from(target),
    }
}

fn same_file(a: &Path, b: &Path) -> bool {
    matches!((fs::canonicalize(a), fs::canonicalize(b)), (Ok(a), Ok(b)) if a == b)
}

/// Removes the file or symlink at `path`. On Windows a symlink to a
/// directory is itself a kind of directory, and only `remove_dir` takes it.
fn remove_link(path: &Path, meta: &fs::Metadata) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(_) if meta.is_symlink() && cfg!(windows) => fs::remove_dir(path),
        result => result,
    }
}

/// Creates a symlink at `link` pointing to `target`.
#[cfg(unix)]
pub(crate) fn symlink(target: &Path, link: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

/// Creates a symlink at `link` pointing to `target`. Windows has separate
/// file and directory symlinks, so the target's kind decides which.
#[cfg(windows)]
pub(crate) fn symlink(target: &Path, link: &Path) -> io::Result<()> {
    let base = link.parent().unwrap_or(Path::new(""));
    if base.join(target).is_dir() {
        std::os::windows::fs::symlink_dir(target, link)
    } else {
        std::os::windows::fs::symlink_file(target, link)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::test_support::{run, scratch_dir};

    #[test]
    fn makes_a_hard_link() {
        let dir = scratch_dir("ln_hard");
        let (target, link) = (dir.join("a.txt"), dir.join("b.txt"));
        fs::write(&target, "one").unwrap();

        let out = run(
            "ln",
            &[target.to_str().unwrap(), link.to_str().unwrap()],
            "",
        );
        assert_eq!(out.status, 0, "{}", out.stderr);
        // Both names are the same file: a write through one shows in both.
        fs::write(&target, "two").unwrap();
        assert_eq!(fs::read_to_string(&link).unwrap(), "two");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn links_several_targets_into_a_directory() {
        let dir = scratch_dir("ln_into_dir");
        let dest = dir.join("dest");
        fs::create_dir(&dest).unwrap();
        fs::write(dir.join("a"), "a").unwrap();
        fs::write(dir.join("b"), "b").unwrap();

        let (a, b) = (dir.join("a"), dir.join("b"));
        let args = [
            a.to_str().unwrap(),
            b.to_str().unwrap(),
            dest.to_str().unwrap(),
        ];
        let out = run("ln", &args, "");
        assert_eq!(out.status, 0, "{}", out.stderr);
        assert_eq!(fs::read_to_string(dest.join("a")).unwrap(), "a");
        assert_eq!(fs::read_to_string(dest.join("b")).unwrap(), "b");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn an_existing_link_name_needs_dash_f() {
        let dir = scratch_dir("ln_force");
        let (target, link) = (dir.join("new"), dir.join("old"));
        fs::write(&target, "new").unwrap();
        fs::write(&link, "old").unwrap();
        let args = [target.to_str().unwrap(), link.to_str().unwrap()];

        let out = run("ln", &args, "");
        assert_eq!(out.status, 1);
        assert!(
            out.stderr.contains("failed to create hard link"),
            "{}",
            out.stderr
        );
        assert_eq!(fs::read_to_string(&link).unwrap(), "old");

        let out = run("ln", &["-f", args[0], args[1]], "");
        assert_eq!(out.status, 0, "{}", out.stderr);
        assert_eq!(fs::read_to_string(&link).unwrap(), "new");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn dash_f_will_not_replace_a_file_with_itself() {
        let dir = scratch_dir("ln_same");
        let file = dir.join("a.txt");
        fs::write(&file, "keep").unwrap();
        let file = file.to_str().unwrap();

        let out = run("ln", &["-f", file, file], "");
        assert_eq!(out.status, 1);
        assert!(out.stderr.contains("are the same file"), "{}", out.stderr);
        assert_eq!(fs::read_to_string(file).unwrap(), "keep");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn dash_s_makes_a_relative_symlink() {
        let dir = scratch_dir("ln_symbolic");
        fs::write(dir.join("a.txt"), "data").unwrap();
        let link = dir.join("link.txt");

        let out = run("ln", &["-s", "a.txt", link.to_str().unwrap()], "");
        if out.stderr.contains("os error 1314") {
            // Windows without Developer Mode can't create symlinks.
            eprintln!("skipping: cannot create a symlink here");
            return;
        }
        assert_eq!(out.status, 0, "{}", out.stderr);
        // Stored as given, and resolved from the link's own directory.
        assert_eq!(fs::read_link(&link).unwrap().to_str(), Some("a.txt"));
        assert_eq!(fs::read_to_string(&link).unwrap(), "data");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn rejects_a_missing_or_bad_destination() {
        let out = run("ln", &[], "");
        assert_eq!(out.status, 1);
        assert!(out.stderr.contains("missing file operand"));

        let out = run("ln", &["a", "b", "no-such-dir"], "");
        assert_eq!(out.status, 1);
        assert!(out
            .stderr
            .contains("target 'no-such-dir' is not a directory"));
    }
}
