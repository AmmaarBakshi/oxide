use crate::{Ctx, Io};

pub fn run(args: &[String], io: &mut Io<'_>, ctx: &mut Ctx<'_>) -> i32 {
    if args.is_empty() {
        // HashMap order is random; sort so the listing is stable.
        let mut aliases: Vec<_> = ctx.aliases.iter().collect();
        aliases.sort();
        for (key, val) in aliases {
            wln!(io, "alias {}='{}'", key, val);
        }
        return 0;
    }

    let mut status = 0;
    for arg in args {
        if let Some((key, value)) = arg.split_once('=') {
            if !is_valid_name(key) {
                ewln!(io, "oxide: alias: '{}': invalid alias name", key);
                status = 1;
                continue;
            }
            let clean_value = value.trim_matches(|c| c == '"' || c == '\'');
            ctx.aliases.insert(key.to_string(), clean_value.to_string());
        } else if let Some(val) = ctx.aliases.get(arg.as_str()) {
            // A bare name shows that one alias.
            wln!(io, "alias {}='{}'", arg, val);
        } else {
            ewln!(io, "oxide: alias: {}: not found", arg);
            status = 1;
        }
    }
    status
}

/// Whether `name` could ever be expanded. The executor only matches the
/// first whitespace-separated word of a line, so an empty name or one with
/// blanks never fires; quotes, `$`, `/` and shell metacharacters are refused
/// the same way bash refuses them.
fn is_valid_name(name: &str) -> bool {
    !name.is_empty()
        && !name
            .chars()
            .any(|c| c.is_whitespace() || "/$`'\"\\|&;()<>".contains(c))
}

#[cfg(test)]
mod tests {
    use crate::test_support::run_with_aliases;

    const ALIASES: [(&str, &str); 3] = [("ll", "ls -a"), ("g", "grep"), ("cls", "clear")];

    #[test]
    fn lists_aliases_sorted() {
        let out = run_with_aliases("alias", &[], &ALIASES);
        assert_eq!(out.stdout, "alias cls='clear'\nalias g='grep'\nalias ll='ls -a'\n");
    }

    #[test]
    fn a_bare_name_shows_one_alias() {
        let out = run_with_aliases("alias", &["ll"], &ALIASES);
        assert_eq!((out.status, out.stdout.as_str()), (0, "alias ll='ls -a'\n"));

        let out = run_with_aliases("alias", &["nope"], &ALIASES);
        assert_eq!(out.status, 1);
        assert!(out.stderr.contains("nope: not found"));
    }

    #[test]
    fn rejects_names_that_could_never_expand() {
        for bad in ["=ls", "my alias=ls", "a/b=ls", "$x=ls", "a|b=ls"] {
            let out = run_with_aliases("alias", &[bad, "ok=ls", "ok"], &[]);
            assert_eq!(out.status, 1, "{:?} was accepted", bad);
            assert!(out.stderr.contains("invalid alias name"), "{}", out.stderr);
            // The bad definition doesn't stop the rest of the line.
            assert_eq!(out.stdout, "alias ok='ls'\n");
        }
    }
}
