use std::env;
use std::path::{self, Path, PathBuf};

use crate::args::parse_flags;
use crate::{registry, Ctx, Io, EXECUTOR_BUILTINS, REPL_BUILTINS};

pub fn run(args: &[String], io: &mut Io<'_>, ctx: &mut Ctx<'_>) -> i32 {
    let (flags, names) = match parse_flags(args, "a") {
        Ok(parsed) => parsed,
        Err(msg) => {
            ewln!(io, "which: {}", msg);
            ewln!(io, "Usage: which [-a] <name>...");
            return 1;
        }
    };
    if names.is_empty() {
        ewln!(io, "which: missing command name (e.g., 'which cargo')");
        return 1;
    }
    let all = flags.contains('a');

    let mut status = 0;
    for name in names {
        // Checked in the order the shell resolves a command, so the first
        // line printed is what would actually run.
        let mut found = false;
        if let Some(expansion) = ctx.aliases.get(name) {
            wln!(io, "{}: aliased to {}", name, expansion);
            found = true;
        }
        if (all || !found) && is_builtin(name) {
            wln!(io, "{}: shell builtin", name);
            found = true;
        }
        if all || !found {
            for path in search_path(name, all) {
                wln!(io, "{}", path.display());
                found = true;
            }
        }
        if !found {
            ewln!(io, "which: no {} in PATH", name);
            status = 1;
        }
    }
    status
}

fn is_builtin(name: &str) -> bool {
    registry().contains(name) || EXECUTOR_BUILTINS.contains(&name) || REPL_BUILTINS.contains(&name)
}

/// The executables `name` resolves to, in search order — only the first
/// unless `all`.
///
/// Mirrors the executor's lookup (`resolve_program_path` in `oxide-exec`):
/// each `PATH` directory is tried for the exact name and then, on Windows,
/// with `.exe` appended. A name containing a path separator is checked as
/// given rather than searched for.
fn search_path(name: &str, all: bool) -> Vec<PathBuf> {
    let dirs: Vec<PathBuf> = if name.contains(path::is_separator) {
        vec![PathBuf::new()]
    } else {
        env::var_os("PATH")
            .map(|paths| env::split_paths(&paths).collect())
            .unwrap_or_default()
    };

    let mut hits = Vec::new();
    for dir in dirs {
        // At most one hit per directory: the executor takes the exact name
        // over `.exe`, so the other could never run from there.
        if let Some(hit) = candidates(&dir, name).into_iter().find(|c| is_executable(c)) {
            hits.push(hit);
            if !all {
                break;
            }
        }
    }
    hits
}

fn candidates(dir: &Path, name: &str) -> Vec<PathBuf> {
    let exact = dir.join(name);
    if cfg!(windows) {
        vec![exact, dir.join(format!("{}.exe", name))]
    } else {
        vec![exact]
    }
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::test_support::{run, run_with_aliases, scratch_dir};

    #[test]
    fn reports_builtins_from_every_tier() {
        for name in ["ls", "jobs", "exit"] {
            let out = run("which", &[name], "");
            assert_eq!(out.status, 0);
            assert_eq!(out.stdout, format!("{}: shell builtin\n", name));
        }
    }

    #[test]
    fn an_alias_shadows_the_builtin_unless_all() {
        let aliases = [("ls", "ls -a")];
        let out = run_with_aliases("which", &["ls"], &aliases);
        assert_eq!(out.stdout, "ls: aliased to ls -a\n");

        let out = run_with_aliases("which", &["-a", "ls"], &aliases);
        assert!(out.stdout.starts_with("ls: aliased to ls -a\nls: shell builtin\n"));
    }

    #[test]
    fn finds_an_executable_given_by_path() {
        let dir = scratch_dir("which_by_path");
        let tool = dir.join("tool");
        fs::write(&tool, "").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&tool, fs::Permissions::from_mode(0o755)).unwrap();
        }

        let out = run("which", &[tool.to_str().unwrap()], "");
        assert_eq!(out.status, 0, "{}", out.stderr);
        assert_eq!(out.stdout, format!("{}\n", tool.display()));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn unknown_names_fail_but_the_rest_are_still_reported() {
        let out = run("which", &["oxide-no-such-command", "cd"], "");
        assert_eq!(out.status, 1);
        assert_eq!(out.stdout, "cd: shell builtin\n");
        assert_eq!(out.stderr, "which: no oxide-no-such-command in PATH\n");
    }
}
