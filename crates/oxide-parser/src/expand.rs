//! Word expansion: what happens to a word between the parser and the command.
//!
//! A word arrives exactly as it was typed, quotes and all. Expanding it:
//!
//! 0. multiplies out unquoted braces, `a{1,2}` to `a1 a2` (see
//!    [`crate::braces`]);
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

    /// The positional parameters, `$1` onwards, for `"$@"`.
    fn positional(&self) -> Vec<String> {
        Vec::new()
    }

    /// Sets a variable, for `$((x = 1))`.
    fn assign(&mut self, _name: &str, _value: &str) {}

    /// Reports an expansion that failed, such as `$((1 / 0))`. The shell
    /// uses this to fail the command instead of running it.
    fn report(&mut self, message: &str) {
        eprintln!("oxide: {}", message);
    }
}

/// Lets arithmetic read and assign through an expansion context.
struct ArithVars<'c>(&'c mut dyn Context);

impl crate::arith::Vars for ArithVars<'_> {
    fn get(&self, name: &str) -> Option<String> {
        self.0.lookup(name)
    }

    fn set(&mut self, name: &str, value: i64) {
        self.0.assign(name, &value.to_string());
    }
}

/// Evaluates the body of a `$(( ))`. `$` expansions in it are replaced
/// first, so `$x`, `$1` and `$(cmd)` all work alongside bare names.
fn arithmetic(body: &str, ctx: &mut dyn Context) -> String {
    let text = expand_string(body, ctx);
    match crate::arith::evaluate(&text, &mut ArithVars(ctx)) {
        Ok(value) => value.to_string(),
        Err(e) => {
            ctx.report(&format!("{}: {}", text.trim(), e));
            String::new()
        }
    }
}

/// Expands `word` into the fields it stands for: none (`$UNSET`), one, or
/// several (`$LIST`, `*.rs`).
pub fn expand_word(word: &str, ctx: &mut dyn Context) -> Vec<String> {
    let mut fields = Vec::new();
    for word in crate::braces::expand(word) {
        for field in expand(&word, true, ctx) {
            if field.has_glob {
                let matches = crate::glob::matches(&field.pattern);
                if !matches.is_empty() {
                    fields.extend(matches);
                    continue;
                }
            }
            fields.push(field.text);
        }
    }
    fields
}

/// Expands `word` into a glob pattern, for `case`: `$` expansions are
/// replaced, and quoted characters, which must match literally, come out
/// escaped.
pub fn expand_pattern(word: &str, ctx: &mut dyn Context) -> String {
    expand(word, false, ctx)
        .into_iter()
        .next()
        .map(|field| field.pattern)
        .unwrap_or_default()
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

/// Expands a here-document body: `$` expansions and command substitutions
/// are replaced, and a backslash escapes `$`, `` ` ``, `\` or a newline.
/// Everything else, quotes included, is kept as written.
pub fn expand_heredoc(body: &str, ctx: &mut dyn Context) -> String {
    let mut out = String::new();
    let mut chars = body.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.peek() {
                Some('\n') => {
                    chars.next();
                }
                Some(&next @ ('$' | '`' | '\\')) => {
                    chars.next();
                    out.push(next);
                }
                _ => out.push(c),
            },
            '$' => match dollar(&mut chars, ctx) {
                Dollar::Value(text) | Dollar::Literal(text) => out.push_str(&text),
                Dollar::Fields(params) => out.push_str(&params.join(" ")),
            },
            '`' => {
                let command = backtick_body(&mut chars);
                out.push_str(ctx.substitute(&command).trim_end_matches(['\n', '\r']));
            }
            _ => out.push(c),
        }
    }
    out
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

    /// Forgets that the current field was started, if nothing is in it
    /// yet: `"$@"` with no parameters is no word at all, not an empty one.
    fn unstart_if_empty(&mut self) {
        if self.current.text.is_empty() {
            self.started = false;
        }
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
                // The opening quote makes a word, even an empty one.
                if in_double {
                    out.start();
                }
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
                // `$@`: each parameter its own word, even inside quotes.
                Dollar::Fields(params) => {
                    if params.is_empty() && in_double {
                        out.unstart_if_empty();
                    }
                    for (i, param) in params.iter().enumerate() {
                        if i > 0 {
                            out.end_field();
                            if in_double {
                                out.start();
                            }
                        }
                        out.push_value(param, in_double);
                    }
                }
            },
            // `` `command` ``: the older spelling of `$(command)`.
            '`' => {
                let command = backtick_body(&mut chars);
                let output = ctx.substitute(&command);
                out.push_value(output.trim_end_matches(['\n', '\r']), in_double);
            }
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
    /// `$@`: the positional parameters, as separate words.
    Fields(Vec<String>),
}

/// Reads what follows a `$` (already consumed) and expands it.
fn dollar(chars: &mut Peekable<Chars<'_>>, ctx: &mut dyn Context) -> Dollar {
    match chars.peek() {
        // `$?`: the exit status of the last command; `$$`: the shell's pid;
        // `$!`: the pid of the last background job; `$#`: how many
        // positional parameters; `$*`: all of them in one string; `$0`..`$9`:
        // one of them (only one digit: `$10` is `$1` then a 0).
        Some(&c @ ('?' | '$' | '!' | '#' | '*' | '0'..='9')) => {
            chars.next();
            Dollar::Value(lookup(&c.to_string(), ctx))
        }
        Some('@') => {
            chars.next();
            Dollar::Fields(ctx.positional())
        }
        // `$(command)`: what the command writes to stdout, without the
        // trailing newlines. `$((expr))` is arithmetic instead.
        Some('(') => {
            chars.next();
            if chars.peek() == Some(&'(') {
                chars.next();
                let body = substitution_body(chars);
                // The inner `(` closed above; this is the outer one's `)`.
                if chars.peek() == Some(&')') {
                    chars.next();
                }
                return Dollar::Value(arithmetic(&body, ctx));
            }
            let command = substitution_body(chars);
            let output = ctx.substitute(&command);
            Dollar::Value(output.trim_end_matches(['\n', '\r']).to_string())
        }
        Some('{') => {
            chars.next();
            match braced_body(chars) {
                Err(partial) => Dollar::Literal(format!("${{{}", partial)),
                Ok(body) if body.is_empty() => Dollar::Literal("${}".to_string()),
                Ok(body) => parameter(&body, ctx),
            }
        }
        Some(&c) if c.is_alphabetic() || c == '_' => {
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

/// Reads the inside of a `${...}` whose `${` is already consumed, up to the
/// `}` that balances it. Quoted text and nested `${...}` are passed over.
/// Returns what was read as the error if the braces never close.
fn braced_body(chars: &mut Peekable<Chars<'_>>) -> Result<String, String> {
    let mut body = String::new();
    let mut depth = 1;
    let mut quote = None;
    while let Some(c) = chars.next() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '\'' | '"') => quote = Some(c),
            (None, '\\') => {
                body.push(c);
                if let Some(next) = chars.next() {
                    body.push(next);
                }
                continue;
            }
            (None, '{') => depth += 1,
            (None, '}') => {
                depth -= 1;
                if depth == 0 {
                    return Ok(body);
                }
            }
            _ => {}
        }
        body.push(c);
    }
    Err(body)
}

/// Splits a `${...}` body into the parameter's name and whatever follows
/// it: a name, a run of digits, or one special character.
fn split_parameter(body: &str) -> (&str, &str) {
    let len = match body.chars().next() {
        Some(c) if c.is_alphabetic() || c == '_' => body
            .find(|c: char| !(c.is_alphanumeric() || c == '_'))
            .unwrap_or(body.len()),
        Some(c) if c.is_ascii_digit() => body
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(body.len()),
        Some('?' | '$' | '!' | '#' | '@' | '*') => 1,
        _ => 0,
    };
    body.split_at(len)
}

/// Expands the inside of a `${...}`: a parameter, optionally followed by
/// an operator and a word.
///
/// - `${#NAME}`: the length of NAME's value, in characters;
/// - `${NAME:-word}`: the word if NAME is unset or empty;
/// - `${NAME:=word}`: the same, and NAME is set to it;
/// - `${NAME%pattern}` / `${NAME%%pattern}`: the value without the
///   shortest / longest suffix matching the glob pattern;
/// - `${NAME#pattern}` / `${NAME##pattern}`: the same for a prefix;
///
/// and `-` / `=` without the colon, which act only when NAME is unset.
fn parameter(body: &str, ctx: &mut dyn Context) -> Dollar {
    // `${#NAME}`. (A bare `${#}` is the number of positional parameters.)
    if let Some(after) = body.strip_prefix('#') {
        let (name, rest) = split_parameter(after);
        if !name.is_empty() && rest.is_empty() {
            let length = lookup(name, ctx).chars().count();
            return Dollar::Value(length.to_string());
        }
    }

    let (name, rest) = split_parameter(body);
    if name.is_empty() {
        ctx.report(&format!("${{{}}}: bad substitution", body));
        return Dollar::Value(String::new());
    }
    let value = get(name, ctx);
    if rest.is_empty() {
        return Dollar::Value(value.unwrap_or_default());
    }

    // With a colon the operator also treats an empty value as missing.
    let (colon, op_rest) = match rest.strip_prefix(':') {
        Some(after) => (true, after),
        None => (false, rest),
    };
    let missing = match &value {
        None => true,
        Some(v) => colon && v.is_empty(),
    };
    let mut op = op_rest.chars();
    let operator = op.next();
    let word = op.as_str();
    match operator {
        Some('-') => Dollar::Value(if missing {
            expand_string(word, ctx)
        } else {
            value.unwrap_or_default()
        }),
        Some('=') => {
            if missing {
                let default = expand_string(word, ctx);
                ctx.assign(name, &default);
                Dollar::Value(default)
            } else {
                Dollar::Value(value.unwrap_or_default())
            }
        }
        Some(c @ ('%' | '#')) if !colon => {
            // Doubled (`%%`, `##`), the longest match goes.
            let (longest, pattern) = match word.strip_prefix(c) {
                Some(rest) => (true, rest),
                None => (false, word),
            };
            let pattern = expand_pattern(pattern, ctx);
            let value = value.unwrap_or_default();
            Dollar::Value(remove_match(&value, &pattern, c == '%', longest))
        }
        _ => {
            ctx.report(&format!("${{{}}}: bad substitution", body));
            Dollar::Value(String::new())
        }
    }
}

/// `value` without the shortest or longest suffix (or prefix) that matches
/// the glob `pattern`; all of `value` if nothing matches.
fn remove_match(value: &str, pattern: &str, suffix: bool, longest: bool) -> String {
    let Ok(pattern) = glob::Pattern::new(pattern) else {
        return value.to_string();
    };
    // Every place the value can be cut, shortest piece first.
    let mut cuts: Vec<usize> = value
        .char_indices()
        .map(|(i, _)| i)
        .chain(std::iter::once(value.len()))
        .collect();
    if suffix {
        cuts.reverse();
    }
    if longest {
        cuts.reverse();
    }
    for cut in cuts {
        if suffix && pattern.matches(&value[cut..]) {
            return value[..cut].to_string();
        }
        if !suffix && pattern.matches(&value[..cut]) {
            return value[cut..].to_string();
        }
    }
    value.to_string()
}

/// Reads the body of a `` `...` `` whose opening backtick is already
/// consumed, up to the closing one. A backslash before `` ` ``, `\` or `$`
/// stands for that character; before anything else it is kept.
fn backtick_body(chars: &mut Peekable<Chars<'_>>) -> String {
    let mut body = String::new();
    while let Some(c) = chars.next() {
        match c {
            '`' => break,
            '\\' => match chars.peek() {
                Some(&next @ ('`' | '\\' | '$')) => {
                    chars.next();
                    body.push(next);
                }
                _ => body.push(c),
            },
            _ => body.push(c),
        }
    }
    body
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
    get(name, ctx).unwrap_or_default()
}

/// A variable's value, or `None` when it is unset.
fn get(name: &str, ctx: &dyn Context) -> Option<String> {
    if name == "$" {
        // Every command runs inside this one process, subshells included,
        // so its id is the shell's id everywhere.
        return Some(std::process::id().to_string());
    }
    ctx.lookup(name)
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

    /// The environment plus a fixed `$?`, and assignments kept to itself.
    struct Status(i32);

    /// Shell variables for the parameter-expansion tests.
    #[derive(Default)]
    struct Vars(std::collections::HashMap<String, String>);

    impl Context for Vars {
        fn lookup(&self, name: &str) -> Option<String> {
            self.0.get(name).cloned()
        }

        fn substitute(&mut self, command: &str) -> String {
            format!("<{}>", command)
        }

        fn assign(&mut self, name: &str, value: &str) {
            self.0.insert(name.to_string(), value.to_string());
        }
    }

    fn vars(pairs: &[(&str, &str)]) -> Vars {
        Vars(
            pairs
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        )
    }

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
    fn backticks_substitute_like_dollar_parens() {
        assert_eq!(string("x`a b`y"), "x<a b>y");
        assert_eq!(words("`a b`"), ["<a", "b>"]);
        assert_eq!(words("\"`a  b`\""), ["<a  b>"]);
        assert_eq!(string("`echo \\`inner\\``"), "<echo `inner`>");
        assert_eq!(string("'`not run`'"), "`not run`");
    }

    #[test]
    fn command_substitution_body_balances_parentheses_and_quotes() {
        assert_eq!(string("$(a (b) c)"), "<a (b) c>");
        assert_eq!(string("$(echo ')' \")\")"), "<echo ')' \")\">");
        assert_eq!(string("\"$(echo \"x y\")\""), "<echo \"x y\">");
    }

    #[test]
    fn heredoc_bodies_expand_dollars_but_keep_quotes() {
        env::set_var("OXIDE_TEST_HEREDOC", "v");
        let mut ctx = Status(0);
        assert_eq!(
            expand_heredoc("'$OXIDE_TEST_HEREDOC' \"$(x)\" \\$no `y`\n", &mut ctx),
            "'v' \"<x>\" $no <y>\n"
        );
        assert_eq!(expand_heredoc("a\\\nb \\q\n", &mut ctx), "ab \\q\n");
    }

    #[test]
    fn braces_expand_before_variables() {
        env::set_var("OXIDE_TEST_BRACES", "v");
        assert_eq!(words("${OXIDE_TEST_BRACES}{1,2}"), ["v1", "v2"]);
        // As in bash, a bare name runs on into the braces: `$X{1,2}` is `$X1 $X2`.
        assert!(words("$OXIDE_TEST_BRACES{1,2}").is_empty());
        // Assignments and other single strings are not brace-expanded.
        assert_eq!(string("{1,2}"), "{1,2}");
    }

    /// Positional parameters and nothing else.
    struct Args(Vec<&'static str>);

    impl Context for Args {
        fn lookup(&self, name: &str) -> Option<String> {
            match name {
                "#" => Some(self.0.len().to_string()),
                "*" => Some(self.0.join(" ")),
                _ => name
                    .parse::<usize>()
                    .ok()
                    .and_then(|n| self.0.get(n.checked_sub(1)?))
                    .map(|s| s.to_string()),
            }
        }

        fn substitute(&mut self, _command: &str) -> String {
            String::new()
        }

        fn positional(&self) -> Vec<String> {
            self.0.iter().map(|s| s.to_string()).collect()
        }
    }

    #[test]
    fn positional_parameters() {
        let mut args = Args(vec!["a b", "c"]);
        assert_eq!(expand_word("$1-$2-$3", &mut args), ["a", "b-c-"]);
        assert_eq!(expand_word("$#", &mut args), ["2"]);
        assert_eq!(expand_word("${2}", &mut args), ["c"]);
        assert_eq!(expand_word("$10", &mut args), ["a", "b0"]);
        assert_eq!(expand_word("\"$*\"", &mut args), ["a b c"]);
    }

    #[test]
    fn quoted_dollar_at_keeps_each_parameter_whole() {
        let mut args = Args(vec!["a b", "c"]);
        assert_eq!(expand_word("\"$@\"", &mut args), ["a b", "c"]);
        assert_eq!(expand_word("\"<$@>\"", &mut args), ["<a b", "c>"]);
        assert_eq!(expand_word("$@", &mut args), ["a", "b", "c"]);
        let mut none = Args(vec![]);
        assert!(expand_word("\"$@\"", &mut none).is_empty());
        assert_eq!(expand_word("\"x$@\"", &mut none), ["x"]);
    }

    #[test]
    fn patterns_escape_what_was_quoted() {
        let mut ctx = Status(0);
        assert_eq!(expand_pattern("*.rs", &mut ctx), "*.rs");
        assert_eq!(expand_pattern("'*'.rs", &mut ctx), "[*].rs");
        assert_eq!(expand_pattern("\"a?\"b*", &mut ctx), "a[?]b*");
    }

    #[test]
    fn arithmetic_expansion() {
        env::set_var("OXIDE_TEST_ARITH", "6");
        assert_eq!(string("$((1 + 2 * 3))"), "7");
        assert_eq!(string("[$(( (1 + 2) * OXIDE_TEST_ARITH ))]"), "[18]");
        assert_eq!(string("$(($OXIDE_TEST_ARITH / 4))"), "1");
        assert_eq!(words("\"$((2 ** 4))\" x"), ["16 x"]);
    }

    #[test]
    fn defaults_stand_in_for_missing_values() {
        let mut ctx = vars(&[("SET", "value"), ("EMPTY", "")]);
        assert_eq!(expand_string("${SET:-other}", &mut ctx), "value");
        assert_eq!(expand_string("${EMPTY:-other}", &mut ctx), "other");
        assert_eq!(expand_string("${UNSET:-other}", &mut ctx), "other");
        // Without the colon an empty value counts as set.
        assert_eq!(expand_string("${EMPTY-other}", &mut ctx), "");
        assert_eq!(expand_string("${UNSET-other}", &mut ctx), "other");
        // The default is expanded, and may nest.
        assert_eq!(
            expand_string("${UNSET:-$SET and ${ALSO:-$(cmd)}}", &mut ctx),
            "value and <cmd>"
        );
    }

    #[test]
    fn prefixes_and_suffixes_come_off() {
        let mut ctx = vars(&[("F", "archive.tar.gz"), ("P", "/home/me/notes.txt")]);
        assert_eq!(expand_string("${F%.*}", &mut ctx), "archive.tar");
        assert_eq!(expand_string("${F%%.*}", &mut ctx), "archive");
        assert_eq!(expand_string("${F#*.}", &mut ctx), "tar.gz");
        assert_eq!(expand_string("${F##*.}", &mut ctx), "gz");
        assert_eq!(expand_string("${P##*/}", &mut ctx), "notes.txt");
        assert_eq!(expand_string("${P%/*}", &mut ctx), "/home/me");
        // No match leaves the value alone; a quoted pattern is literal.
        assert_eq!(expand_string("${F%.zip}", &mut ctx), "archive.tar.gz");
        assert_eq!(expand_string("${F%'*'}", &mut ctx), "archive.tar.gz");
    }

    #[test]
    fn hash_gives_the_length() {
        let mut ctx = vars(&[("WORD", "héllo"), ("EMPTY", "")]);
        assert_eq!(expand_string("${#WORD}", &mut ctx), "5");
        assert_eq!(expand_string("${#EMPTY}${#UNSET}", &mut ctx), "00");
    }

    #[test]
    fn assigning_defaults_set_the_variable() {
        let mut ctx = vars(&[("EMPTY", "")]);
        assert_eq!(expand_string("${EMPTY:=filled}", &mut ctx), "filled");
        assert_eq!(expand_string("${NEW=made}", &mut ctx), "made");
        assert_eq!(ctx.0.get("EMPTY").map(String::as_str), Some("filled"));
        assert_eq!(ctx.0.get("NEW").map(String::as_str), Some("made"));
        assert_eq!(expand_string("${NEW:=again}", &mut ctx), "made");
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
