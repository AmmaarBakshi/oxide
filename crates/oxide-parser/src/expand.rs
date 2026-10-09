//! Word expansion: what happens to a word between the parser and the command.
//!
//! A word arrives exactly as it was typed, quotes and all. Expanding it:
//!
//! 1. replaces a leading `~` with the home directory;
//! 2. replaces `$NAME` and `${NAME}` with the variable's value;
//! 3. splits the result of an unquoted `$` expansion into separate fields
//!    at whitespace;
//! 4. matches unquoted `*`, `?` and `[...]` against the filesystem;
//! 5. removes the quotes.
//!
//! Single quotes keep everything literal. Double quotes allow `$` expansion
//! but prevent splitting and globbing. Outside quotes a backslash escapes the
//! next character only when that character means something to the shell, so
//! a Windows path like `C:\Users\me` survives untouched.

use std::env;
use std::iter::Peekable;
use std::str::Chars;

/// Characters a backslash escapes outside quotes. Before anything else the
/// backslash is kept as an ordinary character.
const ESCAPABLE: &str = " \t\\'\"$`*?[]{}|&;<>()#~";

/// Characters a backslash escapes inside double quotes.
const ESCAPABLE_IN_DOUBLE: &str = "\\\"$`";

/// The shell state expansion reads from, and runs commands in.
pub trait Context {
    /// The value of the variable or special parameter `name` (such as `?`),
    /// or `None` when it is unset.
    fn lookup(&self, name: &str) -> Option<String>;

    /// Runs `command` for a `$(...)` substitution and returns what it wrote
    /// to stdout.
    fn substitute(&mut self, command: &str) -> String;
}

/// Expands `word` into the fields it stands for: none (`$UNSET`), one, or
/// several (`$LIST`, `*.rs`).
pub fn expand_word(word: &str, ctx: &mut dyn Context) -> Vec<String> {
    let mut fields = Vec::new();
    for field in expand(word, true, ctx) {
        if field.has_glob {
            let matches = crate::glob::matches(&field.pattern);
            if !matches.is_empty() {
                fields.extend(matches);
                continue;
            }
        }
        fields.push(field.text);
    }
    fields
}

/// Expands `word` to exactly one string, with no field splitting or
/// globbing. Used where a single value is wanted: assignment values,
/// redirection targets and declarations such as `export NAME=value`.
pub fn expand_string(word: &str, ctx: &mut dyn Context) -> String {
    expand(word, false, ctx)
        .into_iter()
        .next()
        .map(|field| field.text)
        .unwrap_or_default()
}

/// One output field under construction.
#[derive(Default)]
struct Field {
    /// The text with quotes removed.
    text: String,
    /// The same text as a glob pattern, with quoted characters escaped.
    pattern: String,
    /// Whether an unquoted `*`, `?` or `[` appeared.
    has_glob: bool,
}

/// Collects characters into fields, tracking which ones were quoted.
struct Fields {
    split: bool,
    done: Vec<Field>,
    current: Field,
    /// Whether `current` exists yet. A field comes into being with its first
    /// character or with a pair of quotes, so `""` is an empty argument while
    /// an unset `$X` is no argument at all.
    started: bool,
}

impl Fields {
    fn new(split: bool) -> Self {
        Self {
            split,
            done: Vec::new(),
            current: Field::default(),
            started: false,
        }
    }

    fn start(&mut self) {
        self.started = true;
    }

    fn push(&mut self, c: char, quoted: bool) {
        self.started = true;
        self.current.text.push(c);
        if quoted {
            self.current
                .pattern
                .push_str(&glob::Pattern::escape(&c.to_string()));
        } else {
            self.current.has_glob |= matches!(c, '*' | '?' | '[');
            self.current.pattern.push(c);
        }
    }

    fn push_str(&mut self, s: &str, quoted: bool) {
        for c in s.chars() {
            self.push(c, quoted);
        }
    }

    /// Adds the value of an expansion. Unquoted, whitespace in it separates
    /// fields (when splitting is on) and glob characters in it stay live.
    fn push_value(&mut self, value: &str, quoted: bool) {
        if quoted {
            self.start();
            self.push_str(value, true);
            return;
        }
        for c in value.chars() {
            if self.split && c.is_whitespace() {
                self.end_field();
            } else {
                self.push(c, false);
            }
        }
    }

    fn end_field(&mut self) {
        if self.started {
            self.done.push(std::mem::take(&mut self.current));
            self.started = false;
        }
    }

    fn finish(mut self) -> Vec<Field> {
        self.end_field();
        self.done
    }
}

fn expand(word: &str, split: bool, ctx: &mut dyn Context) -> Vec<Field> {
    let mut out = Fields::new(split);
    let mut chars = word.chars().peekable();
    let mut in_double = false;

    if chars.peek() == Some(&'~') {
        chars.next();
        match chars.peek() {
            None | Some('/') | Some('\\') => out.push_str(&home(), true),
            Some(_) => out.push('~', false),
        }
    }

    while let Some(c) = chars.next() {
        match c {
            '\'' if !in_double => {
                out.start();
                for c in chars.by_ref() {
                    if c == '\'' {
                        break;
                    }
                    out.push(c, true);
                }
            }
            '"' => {
                in_double = !in_double;
                out.start();
            }
            '\\' => {
                let escapable = if in_double {
                    ESCAPABLE_IN_DOUBLE
                } else {
                    ESCAPABLE
                };
                match chars.peek() {
                    Some(&next) if escapable.contains(next) => {
                        chars.next();
                        out.push(next, true);
                    }
                    _ => out.push('\\', in_double),
                }
            }
            '$' => match dollar(&mut chars, ctx) {
                Dollar::Value(value) => out.push_value(&value, in_double),
                Dollar::Literal(text) => out.push_str(&text, in_double),
            },
            c => out.push(c, in_double),
        }
    }
    out.finish()
}

/// What a `$` turned out to introduce.
enum Dollar {
    /// An expansion, replaced by this value.
    Value(String),
    /// Nothing expandable, so this text stands as written.
    Literal(String),
}

/// Reads what follows a `$` (already consumed) and expands it.
fn dollar(chars: &mut Peekable<Chars<'_>>, ctx: &mut dyn Context) -> Dollar {
    match chars.peek() {
        // `$?`: the exit status of the last command; `$$`: the shell's pid.
        Some(&c @ ('?' | '$')) => {
            chars.next();
            Dollar::Value(lookup(&c.to_string(), ctx))
        }
        // `$(command)`: what the command writes to stdout, without the
        // trailing newlines.
        Some('(') => {
            chars.next();
            let command = substitution_body(chars);
            let output = ctx.substitute(&command);
            Dollar::Value(output.trim_end_matches(['\n', '\r']).to_string())
        }
        Some('{') => {
            chars.next();
            let mut name = String::new();
            let mut closed = false;
            for c in chars.by_ref() {
                if c == '}' {
                    closed = true;
                    break;
                }
                name.push(c);
            }
            if !closed {
                Dollar::Literal(format!("${{{}", name))
            } else if name.is_empty() {
                Dollar::Literal("${}".to_string())
            } else {
                Dollar::Value(lookup(&name, ctx))
            }
        }
        Some(&c) if c.is_alphanumeric() || c == '_' => {
            let mut name = String::new();
            while let Some(&c) = chars.peek() {
                if !(c.is_alphanumeric() || c == '_') {
                    break;
                }
                name.push(c);
                chars.next();
            }
            Dollar::Value(lookup(&name, ctx))
        }
        _ => Dollar::Literal("$".to_string()),
    }
}

/// Reads the body of a `$(...)` whose `$(` is already consumed, up to the
/// `)` that closes it (consumed, not returned). Parentheses inside quotes
/// or nested substitutions don't count.
fn substitution_body(chars: &mut Peekable<Chars<'_>>) -> String {
    let mut body = String::new();
    let mut depth = 1;
    while let Some(c) = chars.next() {
        match c {
            '\'' => {
                body.push(c);
                for c in chars.by_ref() {
                    body.push(c);
                    if c == '\'' {
                        break;
                    }
                }
            }
            '"' => {
                body.push(c);
                double_quoted_body(chars, &mut body);
            }
            '\\' => {
                body.push(c);
                if let Some(next) = chars.next() {
                    body.push(next);
                }
            }
            '(' => {
                depth += 1;
                body.push(c);
            }
            ')' => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
                body.push(c);
            }
            _ => body.push(c),
        }
    }
    body
}

/// Copies the rest of a double-quoted string (its opening quote already
/// copied) onto `body`, including any `$(...)` inside it, which may hold
/// quotes of its own.
fn double_quoted_body(chars: &mut Peekable<Chars<'_>>, body: &mut String) {
    while let Some(c) = chars.next() {
        body.push(c);
        match c {
            '"' => break,
            '\\' => {
                if let Some(next) = chars.next() {
                    body.push(next);
                }
            }
            '$' if chars.peek() == Some(&'(') => {
                chars.next();
                body.push('(');
                body.push_str(&substitution_body(chars));
                body.push(')');
            }
            _ => {}
        }
    }
}

/// A variable's value; unset reads as empty.
fn lookup(name: &str, ctx: &dyn Context) -> String {
    if name == "$" {
        // Every command runs inside this one process, subshells included,
        // so its id is the shell's id everywhere.
        return std::process::id().to_string();
    }
    ctx.lookup(name).unwrap_or_default()
}

fn home() -> String {
    // Windows uses USERPROFILE, Linux/Mac uses HOME
    env::var("HOME")
        .or_else(|_| env::var("USERPROFILE"))
        .unwrap_or_else(|_| "~".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The environment plus a fixed `$?`.
    struct Status(i32);

    impl Context for Status {
        fn lookup(&self, name: &str) -> Option<String> {
            match name {
                "?" => Some(self.0.to_string()),
                _ => env::var(name).ok(),
            }
        }

        /// Echoes the command back in angle brackets, trailing newlines and all.
        fn substitute(&mut self, command: &str) -> String {
            format!("<{}>\n\n", command)
        }
    }

    fn words(word: &str) -> Vec<String> {
        expand_word(word, &mut Status(0))
    }

    fn string(word: &str) -> String {
        expand_string(word, &mut Status(0))
    }

    #[test]
    fn bare_variable_expands() {
        env::set_var("OXIDE_TEST_BARE", "world");
        assert_eq!(string("hello$OXIDE_TEST_BARE"), "helloworld");
    }

    #[test]
    fn braced_variable_expands_mid_word() {
        env::set_var("OXIDE_TEST_BRACE", "mid");
        assert_eq!(words("pre-${OXIDE_TEST_BRACE}-post"), ["pre-mid-post"]);
    }

    #[test]
    fn undefined_variable_becomes_empty() {
        env::remove_var("OXIDE_TEST_MISSING");
        assert_eq!(words("[$OXIDE_TEST_MISSING]"), ["[]"]);
        assert_eq!(words("[${OXIDE_TEST_MISSING}]"), ["[]"]);
        // On its own an unset variable is no word at all...
        assert!(words("$OXIDE_TEST_MISSING").is_empty());
        // ...but quoted it is an empty one.
        assert_eq!(words("\"$OXIDE_TEST_MISSING\""), [""]);
    }

    #[test]
    fn lone_and_unterminated_dollar_are_literal() {
        assert_eq!(string("cost:$"), "cost:$");
        assert_eq!(string("a${b"), "a${b");
        assert_eq!(string("${}"), "${}");
    }

    #[test]
    fn dollar_question_is_the_last_status() {
        assert_eq!(expand_word("$?", &mut Status(3)), ["3"]);
        assert_eq!(expand_string("[${?}]", &mut Status(127)), "[127]");
        assert_eq!(expand_string("'$?'", &mut Status(3)), "$?");
    }

    #[test]
    fn dollar_dollar_is_the_process_id() {
        let pid = std::process::id().to_string();
        assert_eq!(words("$$"), std::slice::from_ref(&pid));
        assert_eq!(string("pid-${$}.log"), format!("pid-{}.log", pid));
        assert_eq!(string("'$$'"), "$$");
    }

    #[test]
    fn command_substitution_is_replaced_by_its_output() {
        assert_eq!(string("x$(echo hi)y"), "x<echo hi>y");
        // Unquoted, the output is split into words; quoted, it is not.
        assert_eq!(words("$(a b)"), ["<a", "b>"]);
        assert_eq!(words("\"$(a  b)\""), ["<a  b>"]);
    }

    #[test]
    fn command_substitution_body_balances_parentheses_and_quotes() {
        assert_eq!(string("$(a (b) c)"), "<a (b) c>");
        assert_eq!(string("$(echo ')' \")\")"), "<echo ')' \")\">");
        assert_eq!(string("\"$(echo \"x y\")\""), "<echo \"x y\">");
    }

    #[test]
    fn single_quotes_are_literal() {
        env::set_var("OXIDE_TEST_SINGLE", "no");
        assert_eq!(words("'$OXIDE_TEST_SINGLE *'"), ["$OXIDE_TEST_SINGLE *"]);
    }

    #[test]
    fn double_quotes_expand_but_do_not_split() {
        env::set_var("OXIDE_TEST_SPLIT", "a  b");
        assert_eq!(words("\"$OXIDE_TEST_SPLIT\""), ["a  b"]);
        assert_eq!(words("$OXIDE_TEST_SPLIT"), ["a", "b"]);
        assert_eq!(string("$OXIDE_TEST_SPLIT"), "a  b");
    }

    #[test]
    fn quoted_parts_join_the_surrounding_word() {
        assert_eq!(words("pre\"mid dle\"'post'"), ["premid dlepost"]);
        assert_eq!(words("NAME=\"two words\""), ["NAME=two words"]);
    }

    #[test]
    fn backslash_escapes_only_special_characters() {
        assert_eq!(words("a\\ b"), ["a b"]);
        assert_eq!(words("\\$HOME"), ["$HOME"]);
        assert_eq!(words("C:\\Users\\me"), ["C:\\Users\\me"]);
        assert_eq!(words("\"say \\\"hi\\\"\""), ["say \"hi\""]);
    }

    #[test]
    fn tilde_expands_only_at_the_start() {
        let home = home();
        assert_eq!(words("~"), std::slice::from_ref(&home));
        assert_eq!(words("~/notes"), [format!("{}/notes", home)]);
        assert_eq!(words("a~b"), ["a~b"]);
        assert_eq!(words("'~'"), ["~"]);
    }

    #[test]
    fn quoted_glob_characters_are_literal() {
        assert_eq!(words("\"*.no-such-ext\""), ["*.no-such-ext"]);
        // An unquoted pattern with no match is also left as written.
        assert_eq!(words("*.no-such-ext"), ["*.no-such-ext"]);
    }
}
