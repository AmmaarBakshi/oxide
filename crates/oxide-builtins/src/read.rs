use std::env;

use crate::export::is_valid_name;
use crate::{Ctx, Io};

pub fn run(args: &[String], io: &mut Io<'_>, ctx: &mut Ctx<'_>) -> i32 {
    let mut raw = false;
    let mut prompt = None;
    let mut names = Vec::new();

    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "-r" => raw = true,
            "-p" => match iter.next() {
                Some(text) => prompt = Some(text),
                None => {
                    ewln!(io, "oxide: read: -p: option requires an argument");
                    return 2;
                }
            },
            "--" => {
                names.extend(iter.by_ref());
                break;
            }
            flag if flag.starts_with('-') && flag.len() > 1 => {
                ewln!(io, "oxide: read: {}: invalid option", flag);
                ewln!(io, "Usage: read [-r] [-p prompt] [name ...]");
                return 2;
            }
            _ => names.push(arg),
        }
    }
    if let Some(bad) = names.iter().find(|name| !is_valid_name(name)) {
        ewln!(io, "oxide: read: '{}': not a valid identifier", bad);
        return 1;
    }

    // The prompt goes to stderr so it shows even when stdout is redirected.
    if let Some(prompt) = prompt {
        let _ = write!(io.stderr, "{}", prompt);
        let _ = io.stderr.flush();
    }

    let Some((line, complete)) = read_line(io, raw) else {
        return 1;
    };

    if names.is_empty() {
        set(ctx, "REPLY", unescape(&line, raw));
    } else {
        let fields = split(&line, names.len(), raw);
        for (i, name) in names.iter().enumerate() {
            set(ctx, name, fields.get(i).cloned().unwrap_or_default());
        }
    }
    // Like other shells, end-of-input before the newline still assigns but
    // reports failure, so `while read line` stops after the last line.
    if complete {
        0
    } else {
        1
    }
}

/// Reads one line of stdin, without its newline. Unless `raw`, a backslash
/// at the end of a line joins the next one onto it. Returns `None` at
/// end-of-input, otherwise the line and whether it ended in a newline.
fn read_line(io: &mut Io<'_>, raw: bool) -> Option<(String, bool)> {
    let mut line = String::new();
    let mut read_any = false;
    loop {
        let mut part = String::new();
        match io.stdin.read_line(&mut part) {
            Ok(0) | Err(_) => return read_any.then_some((line, false)),
            Ok(_) => read_any = true,
        }
        let Some(text) = part.strip_suffix('\n') else {
            line.push_str(&part);
            return Some((line, false));
        };
        let text = text.strip_suffix('\r').unwrap_or(text);
        match text.strip_suffix('\\') {
            Some(joined) if !raw => line.push_str(joined),
            _ => {
                line.push_str(text);
                return Some((line, true));
            }
        }
    }
}

/// Splits `line` into `count` fields at whitespace; the last field takes
/// whatever is left. Unless `raw`, a backslash keeps the next character,
/// even a space, as part of its field.
fn split(line: &str, count: usize, raw: bool) -> Vec<String> {
    let mut fields = Vec::new();
    let mut rest = line.trim_start();
    while fields.len() + 1 < count && !rest.is_empty() {
        let end = field_end(rest, raw);
        fields.push(unescape(&rest[..end], raw));
        rest = rest[end..].trim_start();
    }
    fields.push(unescape(rest.trim_end(), raw));
    fields
}

/// The byte offset of the first whitespace in `text` not escaped by a
/// backslash (escapes only count unless `raw`).
fn field_end(text: &str, raw: bool) -> usize {
    let mut chars = text.char_indices();
    while let Some((i, c)) = chars.next() {
        if c == '\\' && !raw {
            chars.next();
        } else if c.is_whitespace() {
            return i;
        }
    }
    text.len()
}

/// Drops escaping backslashes, unless `raw`.
fn unescape(text: &str, raw: bool) -> String {
    if raw {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(next) = chars.next() {
                out.push(next);
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Sets a variable the way an assignment would: in the environment if it is
/// exported there, otherwise as a shell variable.
fn set(ctx: &mut Ctx<'_>, name: &str, value: String) {
    if env::var_os(name).is_some() {
        env::set_var(name, value);
    } else {
        ctx.vars.insert(name.to_string(), value);
    }
}

#[cfg(test)]
mod tests {
    use crate::test_support::run_with_vars;

    fn vars(args: &[&str], stdin: &str) -> (i32, Vec<(String, String)>) {
        let (out, vars) = run_with_vars("read", args, stdin);
        let mut vars: Vec<_> = vars.into_iter().collect();
        vars.sort();
        (out.status, vars)
    }

    fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
        list.iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn the_last_name_takes_the_rest_of_the_line() {
        assert_eq!(
            vars(&["first", "rest"], "  one two   three  \nnext\n"),
            (0, pairs(&[("first", "one"), ("rest", "two   three")]))
        );
    }

    #[test]
    fn missing_fields_are_empty_and_reply_is_the_default() {
        assert_eq!(
            vars(&["a", "b", "c"], "only\n"),
            (0, pairs(&[("a", "only"), ("b", ""), ("c", "")]))
        );
        assert_eq!(vars(&[], " as is \n"), (0, pairs(&[("REPLY", " as is ")])));
    }

    #[test]
    fn backslashes_escape_unless_raw() {
        assert_eq!(
            vars(&["a", "b"], "one\\ two three\n"),
            (0, pairs(&[("a", "one two"), ("b", "three")]))
        );
        assert_eq!(
            vars(&["-r", "a"], "C:\\path\\\n"),
            (0, pairs(&[("a", "C:\\path\\")]))
        );
        // Without -r a trailing backslash continues onto the next line.
        assert_eq!(
            vars(&["a"], "joined \\\nline\n"),
            (0, pairs(&[("a", "joined line")]))
        );
    }

    #[test]
    fn end_of_input_fails() {
        assert_eq!(vars(&["a"], ""), (1, pairs(&[])));
        // A last line with no newline is still read, but reports the end.
        assert_eq!(vars(&["a"], "last"), (1, pairs(&[("a", "last")])));
    }

    #[test]
    fn the_prompt_goes_to_stderr() {
        let (out, _) = run_with_vars("read", &["-p", "name? ", "x"], "oxide\n");
        assert_eq!((out.stdout.as_str(), out.stderr.as_str()), ("", "name? "));
    }

    #[test]
    fn bad_names_and_options_are_rejected() {
        let (out, _) = run_with_vars("read", &["1x"], "a\n");
        assert_eq!(out.status, 1);
        assert!(out.stderr.contains("not a valid identifier"));
        let (out, _) = run_with_vars("read", &["-z"], "a\n");
        assert_eq!(out.status, 2);
    }
}
