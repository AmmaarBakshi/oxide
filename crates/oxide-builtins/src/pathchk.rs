use std::path::is_separator;

use crate::args::parse_flags;
use crate::{Ctx, Io};

/// POSIX's minimum guarantees: `_POSIX_NAME_MAX` bytes a component, and a
/// path that fits `_POSIX_PATH_MAX` bytes with its terminating NUL.
const POSIX_NAME_MAX: usize = 14;
const POSIX_PATH_MAX: usize = 255;

/// This system's limits, by the same measure.
const NAME_MAX: usize = 255;
#[cfg(windows)]
const PATH_MAX: usize = 259; // MAX_PATH, less the NUL
#[cfg(not(windows))]
const PATH_MAX: usize = 4095;

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    let (flags, names) = match parse_flags(args, "pP") {
        Ok(parsed) => parsed,
        Err(msg) => {
            ewln!(io, "pathchk: {}", msg);
            ewln!(io, "Usage: pathchk [-p] [-P] name...");
            return 1;
        }
    };
    if names.is_empty() {
        ewln!(io, "pathchk: missing operand");
        ewln!(io, "Usage: pathchk [-p] [-P] name...");
        return 1;
    }

    let rules = Rules {
        portable: flags.contains('p'),
        no_leading_dash: flags.contains('P'),
    };
    let mut status = 0;
    for name in names {
        if let Err(msg) = rules.check(name) {
            ewln!(io, "pathchk: {}", msg);
            status = 1;
        }
    }
    status
}

struct Rules {
    /// `-p`: hold names to POSIX's minimum limits and portable characters.
    portable: bool,
    /// `-P`: reject empty names and components starting with `-`.
    no_leading_dash: bool,
}

impl Rules {
    /// Checks `name` in the order coreutils does, reporting the first
    /// problem found.
    fn check(&self, name: &str) -> Result<(), String> {
        // Only a portable name has to use `/` alone; this system's own
        // separators count otherwise.
        let components: Vec<&str> = if self.portable {
            name.split('/').collect()
        } else {
            name.split(is_separator).collect()
        };

        if self.no_leading_dash && components.iter().any(|c| c.starts_with('-')) {
            return Err(format!(
                "leading '-' in a component of file name '{}'",
                name
            ));
        }
        if (self.portable || self.no_leading_dash) && name.is_empty() {
            return Err("empty file name".to_string());
        }
        if self.portable {
            if let Some(c) = name
                .chars()
                .find(|&c| !(c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '_' | '-')))
            {
                return Err(format!(
                    "nonportable character '{}' in file name '{}'",
                    c, name
                ));
            }
        }

        let (name_max, path_max) = if self.portable {
            (POSIX_NAME_MAX, POSIX_PATH_MAX)
        } else {
            (NAME_MAX, PATH_MAX)
        };
        if name.len() > path_max {
            return Err(format!(
                "limit {} exceeded by length {} of file name '{}'",
                path_max,
                name.len(),
                name
            ));
        }
        if let Some(long) = components.iter().find(|c| c.len() > name_max) {
            return Err(format!(
                "limit {} exceeded by length {} of file name component '{}'",
                name_max,
                long.len(),
                long
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{NAME_MAX, PATH_MAX};
    use crate::test_support::run;

    fn check(args: &[&str]) -> (i32, String) {
        let out = run("pathchk", args, "");
        assert_eq!(out.stdout, "");
        (out.status, out.stderr)
    }

    #[test]
    fn accepts_reasonable_names() {
        assert_eq!(check(&["a/b/c.txt", "notes"]), (0, String::new()));
        assert_eq!(check(&["-p", "a/b/c.txt", "x-y_z.1"]), (0, String::new()));
        assert_eq!(check(&["-pP", "dir/file"]), (0, String::new()));
    }

    #[test]
    fn dash_p_rejects_nonportable_characters() {
        assert_eq!(
            check(&["-p", "a*b"]),
            (
                1,
                "pathchk: nonportable character '*' in file name 'a*b'\n".to_string()
            )
        );
        assert_eq!(check(&["-p", "café"]).0, 1);
        assert_eq!(check(&["-p", r"a\b"]).0, 1);
    }

    #[test]
    fn dash_p_holds_names_to_posix_minimum_limits() {
        assert_eq!(
            check(&["-p", "abcdefghijklmnop"]),
            (
                1,
                "pathchk: limit 14 exceeded by length 16 of file name component \
                 'abcdefghijklmnop'\n"
                    .to_string()
            )
        );
        let long = "a/".repeat(130);
        assert_eq!(
            check(&["-p", &long]).1,
            format!(
                "pathchk: limit 255 exceeded by length 260 of file name '{}'\n",
                long
            )
        );
        // Without -p, names past POSIX's limits but within this system's
        // (even Windows' MAX_PATH) are fine.
        let longish = "ab/".repeat(86);
        assert_eq!(check(&["-p", &longish]).0, 1);
        assert_eq!(check(&["abcdefghijklmnop", &longish]), (0, String::new()));
    }

    #[test]
    fn checks_this_systems_limits_by_default() {
        let component = "a".repeat(NAME_MAX + 1);
        assert_eq!(
            check(&[&component]).1,
            format!(
                "pathchk: limit {} exceeded by length {} of file name component '{}'\n",
                NAME_MAX,
                NAME_MAX + 1,
                component
            )
        );
        let path = "a/".repeat(PATH_MAX / 2 + 1);
        assert!(check(&[&path]).1.contains("of file name '"));
    }

    #[test]
    fn dash_cap_p_rejects_leading_dashes_and_empty_names() {
        assert_eq!(
            check(&["-P", "--", "x/-y"]),
            (
                1,
                "pathchk: leading '-' in a component of file name 'x/-y'\n".to_string()
            )
        );
        assert_eq!(
            check(&["-P", ""]),
            (1, "pathchk: empty file name\n".to_string())
        );
        assert_eq!(check(&["-p", ""]).0, 1);
    }

    #[test]
    fn reports_every_bad_name() {
        let (status, stderr) = check(&["-p", "a*", "ok", "b?"]);
        assert_eq!(status, 1);
        assert_eq!(stderr.lines().count(), 2);
    }

    #[test]
    fn needs_a_name() {
        let (status, stderr) = check(&[]);
        assert_eq!(status, 1);
        assert!(stderr.starts_with("pathchk: missing operand"));
    }
}
