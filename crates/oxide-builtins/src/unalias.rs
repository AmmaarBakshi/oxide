use crate::args::parse_flags;
use crate::{Ctx, Io};

pub fn run(args: &[String], io: &mut Io<'_>, ctx: &mut Ctx<'_>) -> i32 {
    let (flags, names) = match parse_flags(args, "a") {
        Ok(parsed) => parsed,
        Err(msg) => {
            ewln!(io, "oxide: unalias: {}", msg);
            ewln!(io, "Usage: unalias [-a] name...");
            return 1;
        }
    };

    if flags.contains('a') {
        ctx.aliases.clear();
        return 0;
    }
    if names.is_empty() {
        ewln!(io, "oxide: unalias: usage: unalias [-a] name...");
        return 1;
    }

    let mut status = 0;
    for name in names {
        if ctx.aliases.remove(name).is_none() {
            ewln!(io, "oxide: unalias: {}: not found", name);
            status = 1;
        }
    }
    status
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::{registry, Ctx, Io};

    /// Runs `unalias` and returns its status and the aliases left behind.
    fn unalias(args: &[&str], aliases: &[(&str, &str)]) -> (i32, Vec<String>) {
        let mut aliases: HashMap<String, String> = aliases
            .iter()
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect();
        let mut vars = HashMap::new();
        let mut ctx = Ctx {
            aliases: &mut aliases,
            history: &[],
            vars: &mut vars,
            is_function: &|_| false,
        };
        let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
        let (mut stdin, mut stdout, mut stderr) = (&b""[..], Vec::new(), Vec::new());
        let mut io = Io::new(&mut stdin, &mut stdout, &mut stderr);
        let status = registry()
            .get("unalias")
            .unwrap()
            .run(&args, &mut io, &mut ctx);

        let mut left: Vec<String> = aliases.into_keys().collect();
        left.sort();
        (status, left)
    }

    const ALIASES: [(&str, &str); 3] = [("ll", "ls -a"), ("g", "grep"), ("cls", "clear")];

    #[test]
    fn removes_the_named_aliases() {
        assert_eq!(
            unalias(&["ll", "g"], &ALIASES),
            (0, vec!["cls".to_string()])
        );
    }

    #[test]
    fn dash_a_removes_them_all() {
        assert_eq!(unalias(&["-a"], &ALIASES), (0, vec![]));
    }

    #[test]
    fn an_unknown_name_fails_but_the_rest_are_removed() {
        let (status, left) = unalias(&["nope", "ll"], &ALIASES);
        assert_eq!(status, 1);
        assert_eq!(left, ["cls", "g"]);
    }

    #[test]
    fn needs_a_name() {
        assert_eq!(unalias(&[], &ALIASES).0, 1);
    }
}
