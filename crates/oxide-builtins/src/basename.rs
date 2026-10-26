use std::path::is_separator;

use crate::{Ctx, Io};

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    let (names, suffix) = match parse_args(args) {
        Ok(parsed) => parsed,
        Err(msg) => {
            ewln!(io, "basename: {}", msg);
            ewln!(
                io,
                "Usage: basename NAME [SUFFIX] | basename -a [-s SUFFIX] NAME..."
            );
            return 1;
        }
    };
    for name in names {
        wln!(io, "{}", basename(name, suffix));
    }
    0
}

/// Splits `args` into the names to strip and the suffix to drop from each.
/// Without `-a` or `-s`, a second operand is the suffix, as in POSIX.
fn parse_args(args: &[String]) -> Result<(Vec<&str>, &str), String> {
    let mut multiple = false;
    let mut suffix = None;
    let mut names = Vec::new();

    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--" => {
                names.extend(iter.by_ref().map(String::as_str));
                break;
            }
            "-a" => multiple = true,
            "-s" => {
                let value = iter.next().ok_or("option requires an argument -- 's'")?;
                suffix = Some(value.as_str());
                multiple = true;
            }
            flag if flag.len() > 1 && flag.starts_with('-') => {
                return Err(format!("invalid option '{}'", flag));
            }
            name => names.push(name),
        }
    }

    match names.len() {
        0 => Err("missing operand".to_string()),
        2 if !multiple => Ok((vec![names[0]], names[1])),
        n if n > 2 && !multiple => Err(format!("extra operand '{}'", names[2])),
        _ => Ok((names, suffix.unwrap_or(""))),
    }
}

/// The last component of `path`, ignoring trailing separators, with
/// `suffix` removed unless that would leave nothing.
fn basename<'a>(path: &'a str, suffix: &str) -> &'a str {
    let trimmed = path.trim_end_matches(is_separator);
    if trimmed.is_empty() {
        // All separators: the root itself. Empty stays empty.
        return &path[..path.len().min(1)];
    }
    let base = match trimmed.rfind(is_separator) {
        Some(i) => &trimmed[i + 1..],
        None => trimmed,
    };
    match base.strip_suffix(suffix) {
        Some(stem) if !stem.is_empty() => stem,
        _ => base,
    }
}

#[cfg(test)]
mod tests {
    use crate::test_support::run;

    #[test]
    fn strips_the_directory_and_trailing_slashes() {
        assert_eq!(run("basename", &["/usr/lib/"], "").stdout, "lib\n");
        assert_eq!(run("basename", &["file"], "").stdout, "file\n");
        assert_eq!(run("basename", &["/"], "").stdout, "/\n");
    }

    #[test]
    fn a_second_operand_is_a_suffix_to_drop() {
        assert_eq!(
            run("basename", &["src/main.rs", ".rs"], "").stdout,
            "main\n"
        );
        // Dropping the suffix may not leave an empty name.
        assert_eq!(run("basename", &[".rs", ".rs"], "").stdout, ".rs\n");
    }

    #[test]
    fn dash_a_and_dash_s_take_many_names() {
        let out = run("basename", &["-s", ".txt", "a/x.txt", "b/y.txt"], "");
        assert_eq!(out.stdout, "x\ny\n");
        let out = run("basename", &["-a", "a/x", "b/y"], "");
        assert_eq!(out.stdout, "x\ny\n");
    }

    #[test]
    fn reports_missing_and_extra_operands() {
        let out = run("basename", &[], "");
        assert_eq!(out.status, 1);
        assert!(out.stderr.contains("missing operand"));
        let out = run("basename", &["a", "b", "c"], "");
        assert_eq!(out.status, 1);
        assert!(out.stderr.contains("extra operand 'c'"));
    }
}
