//! Fish-style autosuggestions.
//!
//! As you type, the most recent history entry that starts with the current
//! line is shown as greyed-out "ghost text" after the cursor. Pressing the
//! Right arrow (or End) at the end of the line accepts it. The heavy lifting
//! is done by rustyline's `HistoryHinter`, which searches the editor's live
//! history; we wrap it so the rest of the crate owns a single hint type.

use rustyline::hint::{Hinter, HistoryHinter};
use rustyline::Context;

pub struct OxideHinter {
    inner: HistoryHinter,
}

impl Default for OxideHinter {
    fn default() -> Self {
        Self::new()
    }
}

impl OxideHinter {
    pub fn new() -> Self {
        Self {
            inner: HistoryHinter::new(),
        }
    }
}

impl Hinter for OxideHinter {
    type Hint = String;

    fn hint(&self, line: &str, pos: usize, ctx: &Context<'_>) -> Option<String> {
        self.inner.hint(line, pos, ctx)
    }
}
