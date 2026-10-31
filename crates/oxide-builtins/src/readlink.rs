use std::fs;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};

use crate::args::parse_flags;
use crate::realpath::display;
use crate::{Ctx, Io};

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    let (flags, paths) = match parse_flags(args, "fnv") {
        Ok(parsed) => parsed,
        Err(msg) => {
            ewln!(io, "readlink: {}", msg);
            ewln!(io, "Usage: readlink [-fnv] FILE...");
            return 1;
        }
    };
    if paths.is_empty() {
        ewln!(io, "readlink: missing operand");
        return 1;
    }
    let canonical = flags.contains('f');
    let newline = if flags.contains('n') { "" } else { "\n" };

    let mut status = 0;
    for path in paths {
        let result = if canonical {
            canonicalize(Path::new(path))
        } else {
            fs::read_link(path)
        };
        match result {
            Ok(resolved) => {
                if write!(io.stdout, "{}{}", display(resolved), newline).is_err() {
                    return 1;
                }
            }
            Err(e) => {
                // Quiet by default, as in coreutils: a plain file isn't a
                // link, and scripts test for that by the status alone.
                if flags.contains('v') {
                    ewln!(io, "readlink: {}: {}", path, e);
                }
                status = 1;
            }
        }
    }
    status
}

/// The absolute path with every link and `..` resolved, like
/// `realpath`, except that the last component doesn't have to exist.
fn canonicalize(path: &Path) -> io::Result<PathBuf> {
    match fs::canonicalize(path) {
        Err(e) if e.kind() == ErrorKind::NotFound => {
            let Some(name) = path.file_name() else {
                return Err(e);
            };
            let parent = match path.parent() {
                Some(parent) if !parent.as_os_str().is_empty() => parent,
                _ => Path::new("."),
            };
            Ok(fs::canonicalize(parent)?.join(name))
        }
        result => result,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::ln::symlink;
    use crate::test_support::{run, scratch_dir};

    #[test]
    fn prints_where_a_link_points() {
        let dir = scratch_dir("readlink_link");
        fs::write(dir.join("a.txt"), "").unwrap();
        let link = dir.join("link");
        if symlink("a.txt".as_ref(), &link).is_err() {
            // Windows without Developer Mode can't create symlinks.
            eprintln!("skipping: cannot create a symlink here");
            return;
        }
        let out = run("readlink", &[link.to_str().unwrap()], "");
        assert_eq!((out.status, out.stdout.as_str()), (0, "a.txt\n"));

        let out = run("readlink", &["-n", link.to_str().unwrap()], "");
        assert_eq!(out.stdout, "a.txt");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_file_that_is_not_a_link_fails_quietly() {
        let dir = scratch_dir("readlink_plain");
        let file = dir.join("a.txt");
        fs::write(&file, "").unwrap();
        let file = file.to_str().unwrap();

        let out = run("readlink", &[file], "");
        assert_eq!((out.status, out.stdout.as_str()), (1, ""));
        assert_eq!(out.stderr, "");

        let out = run("readlink", &["-v", file], "");
        assert!(out.stderr.starts_with("readlink: "), "{}", out.stderr);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn dash_f_resolves_to_an_absolute_path() {
        let dir = scratch_dir("readlink_canonical");
        fs::create_dir(dir.join("sub")).unwrap();
        fs::write(dir.join("a.txt"), "").unwrap();
        let roundabout = dir.join("sub").join("..").join("a.txt");

        let out = run("readlink", &["-f", roundabout.to_str().unwrap()], "");
        assert_eq!(out.status, 0, "{}", out.stderr);
        let printed = out.stdout.trim_end();
        assert!(!printed.contains(".."), "{}", printed);
        assert!(!printed.starts_with(r"\\?\"), "{}", printed);
        assert_eq!(
            fs::canonicalize(printed).unwrap(),
            fs::canonicalize(dir.join("a.txt")).unwrap()
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn dash_f_allows_only_the_last_component_to_be_missing() {
        let dir = scratch_dir("readlink_missing");
        fs::create_dir(dir.join("sub")).unwrap();
        let missing = dir.join("sub").join("..").join("new.txt");

        let out = run("readlink", &["-f", missing.to_str().unwrap()], "");
        assert_eq!(out.status, 0, "{}", out.stderr);
        let printed = std::path::Path::new(out.stdout.trim_end());
        assert_eq!(printed.file_name().unwrap(), "new.txt");
        assert_eq!(
            fs::canonicalize(printed.parent().unwrap()).unwrap(),
            fs::canonicalize(&dir).unwrap()
        );

        let deeper = dir.join("no-such-dir").join("new.txt");
        let out = run("readlink", &["-f", deeper.to_str().unwrap()], "");
        assert_eq!((out.status, out.stdout.as_str()), (1, ""));
        fs::remove_dir_all(dir).unwrap();
    }
}
