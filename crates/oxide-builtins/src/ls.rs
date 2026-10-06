use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::{Ctx, Io};

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    // 1. Separate flags (starting with '-') from paths
    let flags: Vec<&String> = args.iter().filter(|a| a.starts_with('-')).collect();
    let paths: Vec<&String> = args.iter().filter(|a| !a.starts_with('-')).collect();

    let show_all = flags.iter().any(|f| f.contains('a'));

    // 2. Sort operands: files are printed as given, directories are listed.
    // No operands means the current directory.
    let mut status = 0;
    let mut printed_any = false;
    let mut dirs: Vec<PathBuf> = Vec::new();
    if paths.is_empty() {
        dirs.push(env::current_dir().unwrap_or_default());
    }
    for path in &paths {
        match fs::metadata(path) {
            Ok(meta) if meta.is_dir() => dirs.push(PathBuf::from(path)),
            Ok(_) => {
                wln!(io, "{}", path);
                printed_any = true;
            }
            Err(e) => {
                ewln!(io, "oxide: ls: cannot access '{}': {}", path, e);
                status = 1;
            }
        }
    }

    // 3. With several operands, each directory gets a `name:` header.
    let show_headers = paths.len() > 1;
    for dir in &dirs {
        let names = match entries(dir, show_all) {
            Ok(names) => names,
            Err(e) => {
                ewln!(io, "oxide: ls: cannot open directory '{}': {}", dir.display(), e);
                status = 1;
                continue;
            }
        };
        if show_headers {
            if printed_any {
                wln!(io, "");
            }
            wln!(io, "{}:", dir.display());
        }
        for name in names {
            wln!(io, "{}", name);
        }
        printed_any = true;
    }
    status
}

/// The names in `dir`, sorted, with hidden ones dropped unless `show_all`.
fn entries(dir: &Path, show_all: bool) -> io::Result<Vec<String>> {
    let mut names: Vec<String> = fs::read_dir(dir)?
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| show_all || !name.starts_with('.'))
        .collect();

    // read_dir order is up to the filesystem, so sort for stable output.
    // Case-insensitive, so `Cargo.toml` sits beside `crates`.
    names.sort_by_cached_key(|name| name.to_lowercase());
    Ok(names)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::test_support::{run, scratch_dir};

    #[test]
    fn lists_sorted_and_hides_dotfiles_unless_all() {
        let dir = scratch_dir("ls_list");
        for name in ["b.txt", "A.txt", ".hidden"] {
            fs::write(dir.join(name), "").unwrap();
        }
        let arg = dir.to_str().unwrap();

        assert_eq!(run("ls", &[arg], "").stdout, "A.txt\nb.txt\n");
        assert_eq!(run("ls", &["-a", arg], "").stdout, ".hidden\nA.txt\nb.txt\n");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn files_first_then_each_directory_under_a_header() {
        let dir = scratch_dir("ls_operands");
        let (file, sub) = (dir.join("f.txt"), dir.join("sub"));
        fs::write(&file, "").unwrap();
        fs::create_dir(&sub).unwrap();
        fs::write(sub.join("inner.txt"), "").unwrap();
        let (file, sub) = (file.to_str().unwrap(), sub.to_str().unwrap());

        let out = run("ls", &[sub, file], "");
        assert_eq!(out.status, 0, "{}", out.stderr);
        assert_eq!(out.stdout, format!("{}\n\n{}:\ninner.txt\n", file, sub));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn missing_paths_are_reported_without_hiding_the_rest() {
        let dir = scratch_dir("ls_missing");
        let (file, missing) = (dir.join("f.txt"), dir.join("missing"));
        fs::write(&file, "").unwrap();

        let out = run("ls", &[missing.to_str().unwrap(), file.to_str().unwrap()], "");
        assert_eq!(out.status, 1);
        assert_eq!(out.stdout, format!("{}\n", file.to_str().unwrap()));
        assert!(out.stderr.contains("cannot access"));
        fs::remove_dir_all(dir).unwrap();
    }
}
