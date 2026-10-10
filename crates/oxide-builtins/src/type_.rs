use crate::args::parse_flags;
use crate::which::{is_builtin, search_path};
use crate::{Ctx, Io};

/// Words the parser treats as syntax rather than as commands to run.
pub const KEYWORDS: &[&str] = &["if", "elif", "else", "while", "for", "in"];

pub fn run(args: &[String], io: &mut Io<'_>, ctx: &mut Ctx<'_>) -> i32 {
    let (flags, names) = match parse_flags(args, "at") {
        Ok(parsed) => parsed,
        Err(msg) => {
            ewln!(io, "oxide: type: {}", msg);
            ewln!(io, "Usage: type [-a] [-t] <name>...");
            return 2;
        }
    };
    if names.is_empty() {
        ewln!(io, "Usage: type [-a] [-t] <name>...");
        return 2;
    }
    // -a: every meaning a name has, not just the one that wins.
    // -t: one word for the kind (alias, keyword, function, builtin, file).
    let all = flags.contains('a');
    let terse = flags.contains('t');

    let mut status = 0;
    for name in names {
        let mut found = Vec::new();
        if let Some(expansion) = ctx.aliases.get(name) {
            found.push(("alias", format!("{} is aliased to `{}'", name, expansion)));
        }
        if KEYWORDS.contains(&name) {
            found.push(("keyword", format!("{} is a shell keyword", name)));
        }
        if (ctx.is_function)(name) {
            found.push(("function", format!("{} is a function", name)));
        }
        if is_builtin(name) {
            found.push(("builtin", format!("{} is a shell builtin", name)));
        }
        for path in search_path(name, all) {
            found.push(("file", format!("{} is {}", name, path.display())));
        }

        if found.is_empty() {
            if !terse {
                ewln!(io, "oxide: type: {}: not found", name);
            }
            status = 1;
            continue;
        }
        let shown = if all { found.len() } else { 1 };
        for (kind, description) in found.into_iter().take(shown) {
            if terse {
                wln!(io, "{}", kind);
            } else {
                wln!(io, "{}", description);
            }
        }
    }
    status
}

#[cfg(test)]
mod tests {
    use crate::test_support::{run, run_with_aliases};

    #[test]
    fn describes_builtins_keywords_and_aliases() {
        let out = run("type", &["cd", "if", "exit"], "");
        assert_eq!(out.status, 0, "{}", out.stderr);
        assert_eq!(
            out.stdout,
            "cd is a shell builtin\nif is a shell keyword\nexit is a shell builtin\n"
        );
        let out = run_with_aliases("type", &["ll"], &[("ll", "ls -a")]);
        assert_eq!(out.stdout, "ll is aliased to `ls -a'\n");
    }

    #[test]
    fn functions_come_before_builtins() {
        let out = run("type", &["-t", "test_support_fn"], "");
        assert_eq!(out.stdout, "function\n");
    }

    #[test]
    fn terse_prints_just_the_kind() {
        let out = run_with_aliases("type", &["-t", "ll", "for", "pwd"], &[("ll", "ls")]);
        assert_eq!(out.stdout, "alias\nkeyword\nbuiltin\n");
    }

    #[test]
    fn all_shows_what_an_alias_hides() {
        let out = run_with_aliases("type", &["-a", "ls"], &[("ls", "ls -a")]);
        assert!(
            out.stdout
                .starts_with("ls is aliased to `ls -a'\nls is a shell builtin\n"),
            "{}",
            out.stdout
        );
    }

    #[test]
    fn unknown_names_fail_but_the_rest_are_reported() {
        let out = run("type", &["oxide-no-such-thing", "cd"], "");
        assert_eq!(out.status, 1);
        assert_eq!(out.stdout, "cd is a shell builtin\n");
        assert_eq!(out.stderr, "oxide: type: oxide-no-such-thing: not found\n");
    }
}
