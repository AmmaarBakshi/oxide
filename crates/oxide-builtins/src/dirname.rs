use std::path::is_separator;

use crate::args::parse_flags;
use crate::{Ctx, Io};

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    let (_, names) = match parse_flags(args, "") {
        Ok(parsed) => parsed,
        Err(msg) => {
            ewln!(io, "dirname: {}", msg);
            ewln!(io, "Usage: dirname NAME...");
            return 1;
        }
    };
    if names.is_empty() {
        ewln!(io, "dirname: missing operand");
        return 1;
    }
    for name in names {
        wln!(io, "{}", dirname(name));
    }
    0
}

/// Everything before the last component of `path`, or `.` when there is no
/// directory part. Purely textual: the path need not exist.
fn dirname(path: &str) -> &str {
    let trimmed = path.trim_end_matches(is_separator);
    if trimmed.is_empty() {
        return if path.is_empty() { "." } else { &path[..1] };
    }
    match trimmed.rfind(is_separator) {
        None => ".",
        Some(i) => {
            let parent = trimmed[..i].trim_end_matches(is_separator);
            if parent.is_empty() {
                &trimmed[..1]
            } else {
                parent
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::test_support::run;

    #[test]
    fn strips_the_last_component() {
        let out = run("dirname", &["/usr/lib/", "a/b//c", "/top"], "");
        assert_eq!(out.stdout, "/usr\na/b\n/\n");
    }

    #[test]
    fn a_bare_name_lives_in_dot() {
        assert_eq!(run("dirname", &["file.txt", ""], "").stdout, ".\n.\n");
    }

    #[test]
    fn requires_an_operand() {
        let out = run("dirname", &[], "");
        assert_eq!(out.status, 1);
        assert!(out.stderr.contains("missing operand"));
    }
}
