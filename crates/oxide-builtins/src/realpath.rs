use std::fs;
use std::path::PathBuf;

use crate::args::parse_flags;
use crate::{Ctx, Io};

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    let (_, paths) = match parse_flags(args, "") {
        Ok(parsed) => parsed,
        Err(msg) => {
            ewln!(io, "realpath: {}", msg);
            ewln!(io, "Usage: realpath PATH...");
            return 1;
        }
    };
    if paths.is_empty() {
        ewln!(io, "realpath: missing operand");
        return 1;
    }

    let mut status = 0;
    for path in paths {
        match fs::canonicalize(path) {
            Ok(resolved) => wln!(io, "{}", display(resolved)),
            Err(e) => {
                ewln!(io, "realpath: {}: {}", path, e);
                status = 1;
            }
        }
    }
    status
}

/// Drops the `\\?\` prefix Windows puts on canonical paths, which nothing
/// else in the shell prints and most programs don't accept back.
pub(crate) fn display(path: PathBuf) -> String {
    let text = path.to_string_lossy();
    if let Some(unc) = text.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{}", unc)
    } else if let Some(local) = text.strip_prefix(r"\\?\") {
        local.to_string()
    } else {
        text.into_owned()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::test_support::{run, scratch_dir};

    #[test]
    fn resolves_dot_dot_to_an_absolute_path() {
        let dir = scratch_dir("realpath_resolve");
        fs::create_dir(dir.join("sub")).unwrap();
        let roundabout = dir.join("sub").join("..").join("sub");

        let out = run("realpath", &[roundabout.to_str().unwrap()], "");
        assert_eq!(out.status, 0, "{}", out.stderr);
        let printed = out.stdout.trim_end();
        assert!(!printed.contains(".."), "{}", printed);
        assert!(!printed.starts_with(r"\\?\"), "{}", printed);
        assert_eq!(
            fs::canonicalize(printed).unwrap(),
            fs::canonicalize(dir.join("sub")).unwrap()
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_missing_path_is_reported_and_the_rest_still_print() {
        let dir = scratch_dir("realpath_missing");
        let missing = dir.join("nope");

        let out = run(
            "realpath",
            &[missing.to_str().unwrap(), dir.to_str().unwrap()],
            "",
        );
        assert_eq!(out.status, 1);
        assert_eq!(out.stdout.lines().count(), 1);
        assert!(out.stderr.starts_with("realpath: "), "{}", out.stderr);
        fs::remove_dir_all(dir).unwrap();
    }
}
