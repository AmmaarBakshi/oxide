use std::fs;
use std::io::Write;
use std::path::Path;

use crate::{Ctx, Io};

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    // We need at least 2 arguments: the starting path and the search term
    if args.len() < 2 {
        ewln!(io, "oxide: find: missing arguments.");
        ewln!(io, "Usage: find <path> <search_term> (e.g., 'find . .json')");
        return 1;
    }

    let start_path = Path::new(&args[0]);
    let target = &args[1];

    if !start_path.exists() {
        ewln!(io, "oxide: find: path '{}' does not exist", start_path.display());
        return 1;
    }

    match search_directory(start_path, target, io.stdout) {
        Ok(()) => 0,
        // The sink went away mid-walk (closed pipe); stop quietly.
        Err(_) => 1,
    }
}

/// Recursively digs through folders looking for files that match the target.
///
/// Directories we can't read (access denied, races) are skipped silently, but
/// a failed *write* aborts the whole walk — there is no point searching on
/// once the consumer has hung up.
fn search_directory(dir: &Path, target: &str, out: &mut dyn Write) -> std::io::Result<()> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Ok(());
    };

    for entry in entries.flatten() {
        let path = entry.path();

        // `file_type` doesn't follow symlinks (or junctions, on Windows), so
        // a link back up the tree is matched by name like a file instead of
        // being walked again and again.
        let is_dir = entry.file_type().is_ok_and(|t| t.is_dir());
        if is_dir {
            search_directory(&path, target, out)?;
        } else if let Some(file_name) = path.file_name() {
            if file_name.to_string_lossy().contains(target) {
                writeln!(out, "{}", path.display())?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use crate::test_support::{run, scratch_dir};

    #[test]
    fn finds_matching_names_in_subdirectories() {
        let dir = scratch_dir("find_nested");
        fs::create_dir(dir.join("sub")).unwrap();
        fs::write(dir.join("sub").join("data.json"), "").unwrap();
        fs::write(dir.join("notes.txt"), "").unwrap();

        let out = run("find", &[dir.to_str().unwrap(), ".json"], "");
        assert_eq!(out.status, 0, "{}", out.stderr);
        assert_eq!(out.stdout.trim_end(), dir.join("sub").join("data.json").display().to_string());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn does_not_follow_a_symlink_back_up_the_tree() {
        let dir = scratch_dir("find_loop");
        fs::write(dir.join("hit.txt"), "").unwrap();
        if symlink_dir(&dir, &dir.join("loop")).is_err() {
            // Windows without Developer Mode can't create symlinks.
            eprintln!("skipping: cannot create a directory symlink here");
            return;
        }

        let out = run("find", &[dir.to_str().unwrap(), "hit"], "");
        assert_eq!(out.stdout.lines().count(), 1, "{}", out.stdout);
        fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(unix)]
    fn symlink_dir(original: &Path, link: &Path) -> std::io::Result<()> {
        std::os::unix::fs::symlink(original, link)
    }

    #[cfg(windows)]
    fn symlink_dir(original: &Path, link: &Path) -> std::io::Result<()> {
        std::os::windows::fs::symlink_dir(original, link)
    }
}
