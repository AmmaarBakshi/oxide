use std::borrow::Cow;

use rustyline::completion::{Completer, FilenameCompleter, Pair};
use rustyline::error::ReadlineError;
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::validate::Validator;
use rustyline::{Context, Helper};

// This struct acts as the "brain" for Rustyline's advanced features
pub struct OxideHelper {
    pub completer: FilenameCompleter,
}

// 1. Tell it how to Auto-Complete (We just pass the work to the FilenameCompleter!)
impl Completer for OxideHelper {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        ctx: &Context<'_>,
    ) -> Result<(usize, Vec<Pair>), ReadlineError> {
        self.completer.complete(line, pos, ctx)
    }
}

// 2. We leave these empty for now, but we need them to satisfy the Helper trait!
impl Hinter for OxideHelper {
    type Hint = String;
    fn hint(&self, _line: &str, _pos: usize, _ctx: &Context<'_>) -> Option<String> { None }
}
// Rusty-orange (#B7410E) escape code, applied to the leading "oxide" of the
// prompt. We colorize here (via highlight_prompt) rather than baking ANSI into
// the prompt string, so rustyline still measures the prompt's *visible* width
// correctly and the cursor lands in the right place.
const RUST_ORANGE: &str = "\x1b[38;2;183;65;14m";
const RESET: &str = "\x1b[0m";

fn colorize_prompt(prompt: &str) -> Cow<'_, str> {
    match prompt.strip_prefix("oxide") {
        Some(rest) => Cow::Owned(format!("{RUST_ORANGE}oxide{RESET}{rest}")),
        None => Cow::Borrowed(prompt),
    }
}

impl Highlighter for OxideHelper {
    fn highlight_prompt<'b, 's: 'b, 'p: 'b>(
        &'s self,
        prompt: &'p str,
        _default: bool,
    ) -> Cow<'b, str> {
        colorize_prompt(prompt)
    }
}
impl Validator for OxideHelper {}
impl Helper for OxideHelper {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colorizes_leading_oxide() {
        let out = colorize_prompt("oxide C:\\path > ");
        assert_eq!(out, format!("{RUST_ORANGE}oxide{RESET} C:\\path > "));
        // The word "oxide" is wrapped exactly once, and the rest is untouched.
        assert!(out.starts_with(RUST_ORANGE));
        assert!(out.contains("C:\\path > "));
    }

    #[test]
    fn leaves_unrelated_prompt_untouched() {
        let out = colorize_prompt("$ ");
        assert_eq!(out, "$ ");
    }
}