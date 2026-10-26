//! Oxide's built-in commands.
//!
//! Builtins run inside the shell process rather than being spawned, because
//! they either change shell state (`cd`, `export`, `alias`) or are cheap
//! enough that a process spawn dominates the cost (`echo`, `pwd`).
//!
//! Every builtin has the same shape — [`Builtin::run`] takes its arguments, an
//! [`Io`] stream trio, and a [`Ctx`] of shell state, and returns an exit code.
//! Because output goes to `io.stdout` rather than the real stdout, the same
//! implementation works standalone, behind a `>` redirect, and as a stage in a
//! pipeline. See [`registry`] for the table that lists them all.

use std::collections::HashMap;

#[macro_use]
mod macros;

mod args;
pub mod io;
pub mod registry;

pub mod alias;
pub mod basename;
pub mod bind;
pub mod cat;
pub mod cd;
pub mod clear;
pub mod cp;
pub mod cut;
pub mod dirname;
pub mod echo;
pub mod env;
pub mod export;
pub mod find;
pub mod fold;
pub mod grep;
pub mod head;
pub mod help;
pub mod history;
pub mod kill;
pub mod let_;
pub mod ls;
pub mod mkdir;
pub mod mv;
pub mod nl;
pub mod open;
pub mod paste;
pub mod printf;
pub mod ps;
pub mod pwd;
pub mod read;
pub mod realpath;
pub mod rev;
pub mod rm;
pub mod rmdir;
pub mod seq;
pub mod sleep;
pub mod sort;
pub mod tac;
pub mod tail;
pub mod tee;
pub mod test;
pub mod top;
pub mod touch;
pub mod tr;
pub mod truth;
pub mod type_;
pub mod unalias;
pub mod uniq;
pub mod unset;
pub mod wc;
pub mod which;

#[cfg(test)]
mod test_support;

pub use io::Io;
pub use registry::{registry, Builtin, Registry};

/// Shell state a builtin is allowed to read or modify.
///
/// This deliberately holds only plain std types. Builtins that need the
/// executor itself — `source`, `jobs`, `jail` — can't be expressed here and
/// live in `oxide-exec` instead; see [`EXECUTOR_BUILTINS`].
pub struct Ctx<'a> {
    /// The alias table, mutable so `alias` can define new ones.
    pub aliases: &'a mut HashMap<String, String>,
    /// Commands run so far this session, for `history`.
    pub history: &'a [String],
    /// Shell variables: set with `NAME=value` but not exported, so child
    /// processes don't see them. Exported variables live in the process
    /// environment instead.
    pub vars: &'a mut HashMap<String, String>,
    /// Whether a name is a shell function, for `type`. (The functions
    /// themselves live in the executor.)
    pub is_function: &'a dyn Fn(&str) -> bool,
}

/// Builtins that `oxide-exec` dispatches itself because they need executor
/// state (the job table, the compat mode, the script runtime, or recursion
/// back into the parser).
///
/// Listed here so `oxide-ui` can complete and highlight them without
/// depending on `oxide-exec`. `oxide-exec` has a test asserting it really
/// handles every name in this list.
pub const EXECUTOR_BUILTINS: &[&str] = &[
    "break", "call", "continue", "exit", "import", "jail", "jobs", "local", "mode", "refresh",
    "return", "set", "source", "wait",
];

/// Every name Oxide handles internally, from both tiers.
///
/// Derived from the registry rather than hand-maintained, so completion and
/// highlighting cannot drift out of sync with what actually runs.
pub fn all_names() -> Vec<&'static str> {
    let mut names: Vec<&'static str> = registry().names().collect();
    names.extend_from_slice(EXECUTOR_BUILTINS);
    names.sort_unstable();
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_names_covers_both_tiers() {
        let names = all_names();
        assert!(names.contains(&"ls"), "registry tier missing");
        assert!(names.contains(&"jobs"), "executor tier missing");
        assert!(names.contains(&"exit"), "exit is not advertised");
    }

    #[test]
    fn no_name_is_claimed_by_two_tiers() {
        let mut names = all_names();
        let before = names.len();
        names.dedup();
        assert_eq!(before, names.len(), "a builtin name is registered twice");
    }
}
