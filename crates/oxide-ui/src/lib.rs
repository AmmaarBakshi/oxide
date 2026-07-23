//! Oxide's interactive line editor.
//!
//! This crate is the "brain" that sits on top of rustyline and provides the
//! interactive experience: syntax highlighting of the input line, smart
//! tab-completion (builtins + `PATH` executables + files), and fish-style
//! history autosuggestions.
//!
//! The entry point is [`build_editor`], which returns a fully configured
//! rustyline `Editor` for the REPL to drive.

use std::borrow::Cow;

use rustyline::completion::{Completer, Pair};
use rustyline::config::{CompletionType, Config};
use rustyline::error::ReadlineError;
use rustyline::highlight::{CmdKind, Highlighter};
use rustyline::hint::Hinter;
use rustyline::history::DefaultHistory;
use rustyline::validate::Validator;
use rustyline::{Context, Editor, Helper};

pub mod autosuggest;
pub mod completion;
pub mod highlight;
pub mod prompt;

use autosuggest::OxideHinter;
use completion::OxideCompleter;

/// The rustyline editor type used throughout the shell.
pub type OxideEditor = Editor<OxideHelper, DefaultHistory>;

/// Aggregates completion, hinting, and highlighting into the single object
/// rustyline expects (`Helper`).
pub struct OxideHelper {
    completer: OxideCompleter,
    hinter: OxideHinter,
}

impl Default for OxideHelper {
    fn default() -> Self {
        Self::new()
    }
}

impl OxideHelper {
    pub fn new() -> Self {
        Self {
            completer: OxideCompleter::new(),
            hinter: OxideHinter::new(),
        }
    }
}

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

impl Hinter for OxideHelper {
    type Hint = String;

    fn hint(&self, line: &str, pos: usize, ctx: &Context<'_>) -> Option<String> {
        self.hinter.hint(line, pos, ctx)
    }
}

impl Highlighter for OxideHelper {
    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> Cow<'l, str> {
        highlight::highlight_line(line, self.completer.commands())
    }

    fn highlight_prompt<'b, 's: 'b, 'p: 'b>(
        &'s self,
        prompt: &'p str,
        _default: bool,
    ) -> Cow<'b, str> {
        highlight::colorize_prompt(prompt)
    }

    fn highlight_hint<'h>(&self, hint: &'h str) -> Cow<'h, str> {
        Cow::Owned(highlight::colorize_hint(hint))
    }

    /// Ask rustyline to re-highlight on every edit (but not on plain cursor
    /// moves) so colors track the line as it changes.
    fn highlight_char(&self, line: &str, _pos: usize, kind: CmdKind) -> bool {
        !line.is_empty() && kind != CmdKind::MoveCursor
    }
}

impl Validator for OxideHelper {}
impl Helper for OxideHelper {}

/// Build a fully configured editor with Oxide's helper attached.
///
/// History is *not* auto-added; the REPL manages history explicitly so it can
/// also mirror entries to Oxide's own history file.
pub fn build_editor() -> rustyline::Result<OxideEditor> {
    let config = Config::builder()
        .completion_type(CompletionType::List)
        .auto_add_history(false)
        .build();
    let mut editor = Editor::with_config(config)?;
    editor.set_helper(Some(OxideHelper::new()));
    Ok(editor)
}
