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
impl Highlighter for OxideHelper {}
impl Validator for OxideHelper {}
impl Helper for OxideHelper {}