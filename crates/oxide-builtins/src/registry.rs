//! The builtin registry: the single source of truth for which names Oxide
//! handles itself.
//!
//! Everything downstream derives from this table — dispatch in `oxide-exec`,
//! tab-completion and syntax highlighting in `oxide-ui`, and the output of
//! `help`. Adding a builtin means adding one line to [`declare_builtins!`]
//! below; nothing else has to be kept in sync by hand.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use crate::{Ctx, Io};

/// A command Oxide runs in-process instead of spawning.
///
/// Implementors are stateless unit structs; per-invocation state arrives via
/// `io` and `ctx`, which is what lets a single `&'static` registry be shared
/// across every stage of every pipeline.
pub trait Builtin: Send + Sync {
    /// The name this builtin is invoked as.
    fn name(&self) -> &'static str;

    /// One-line description, shown by `help`.
    fn help(&self) -> &'static str;

    /// Runs the builtin and returns its exit status.
    fn run(&self, args: &[String], io: &mut Io<'_>, ctx: &mut Ctx<'_>) -> i32;
}

/// A name-to-builtin lookup table. Ordered so `names()` and `help` come out
/// alphabetically without a separate sort.
pub struct Registry {
    map: BTreeMap<&'static str, Box<dyn Builtin>>,
}

impl Registry {
    fn new() -> Self {
        Self { map: BTreeMap::new() }
    }

    /// Registers a builtin under its own [`Builtin::name`].
    ///
    /// Panics on a duplicate name: two builtins answering to one name is a
    /// build-time mistake, and failing loudly here beats silently shadowing
    /// one of them at runtime.
    fn insert(&mut self, builtin: Box<dyn Builtin>) {
        let name = builtin.name();
        if self.map.insert(name, builtin).is_some() {
            panic!("oxide: duplicate builtin registration for '{name}'");
        }
    }

    pub fn get(&self, name: &str) -> Option<&dyn Builtin> {
        self.map.get(name).map(|b| b.as_ref())
    }

    pub fn contains(&self, name: &str) -> bool {
        self.map.contains_key(name)
    }

    /// Every registered name, alphabetically.
    pub fn names(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.map.keys().copied()
    }

    /// Every registered builtin, alphabetically by name.
    pub fn iter(&self) -> impl Iterator<Item = &dyn Builtin> + '_ {
        self.map.values().map(|b| b.as_ref())
    }
}

/// Generates the unit struct and [`Builtin`] impl for each entry, plus the
/// `install` function that registers them all.
macro_rules! declare_builtins {
    ($( $ty:ident => $name:literal, $run:path, $help:literal ; )*) => {
        $(
            #[doc = concat!("The `", $name, "` builtin.")]
            pub struct $ty;

            impl Builtin for $ty {
                fn name(&self) -> &'static str { $name }
                fn help(&self) -> &'static str { $help }
                fn run(&self, args: &[String], io: &mut Io<'_>, ctx: &mut Ctx<'_>) -> i32 {
                    $run(args, io, ctx)
                }
            }
        )*

        fn install(registry: &mut Registry) {
            $( registry.insert(Box::new($ty)); )*
        }
    };
}

declare_builtins! {
    Alias  => "alias",  crate::alias::run,   "define or list command aliases";
    Cat    => "cat",    crate::cat::run,     "concatenate files, or stdin, to stdout";
    Cd     => "cd",     crate::cd::run,      "change the working directory";
    Clear  => "clear",  crate::clear::run,   "clear the screen and scrollback";
    Dir    => "dir",    crate::ls::run,      "list directory contents (alias for ls)";
    Echo   => "echo",   crate::echo::run,    "write arguments to stdout";
    Env    => "env",    crate::env::run,     "print the environment";
    Export => "export", crate::export::run,  "set an environment variable (KEY=VALUE)";
    Find   => "find",   crate::find::run,    "search a directory tree for matching names";
    Grep   => "grep",   crate::grep::run,    "print lines matching a pattern";
    Head   => "head",   crate::head::run,    "print the first lines of files or stdin";
    Help   => "help",   crate::help::run,    "list the built-in commands";
    Hist   => "history", crate::history::run, "show the command history";
    Kill   => "kill",   crate::kill::run,    "terminate a process by PID";
    Ls     => "ls",     crate::ls::run,      "list directory contents";
    Mkdir  => "mkdir",  crate::mkdir::run,   "create directories";
    Open   => "open",   crate::open::run,    "read a file, parsing JSON and CSV structurally";
    Ps     => "ps",     crate::ps::run,      "list running processes";
    Pwd    => "pwd",    crate::pwd::run,     "print the working directory";
    Rm     => "rm",     crate::rm::run,      "remove files and directories";
    Sleep  => "sleep",  crate::sleep::run,   "pause for a number of seconds";
    Top    => "top",    crate::top::run,     "show the busiest processes";
    Touch  => "touch",  crate::touch::run,   "create files if they do not exist";
    Unset  => "unset",  crate::unset::run,   "remove an environment variable";
}

/// The process-wide builtin registry, built once on first use.
pub fn registry() -> &'static Registry {
    static REGISTRY: OnceLock<Registry> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        let mut registry = Registry::new();
        install(&mut registry);
        registry
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_builds_without_duplicate_names() {
        // `insert` panics on a duplicate, so simply building it is the test.
        assert!(registry().contains("ls"));
    }

    #[test]
    fn every_builtin_reports_the_name_it_is_registered_under() {
        for name in registry().names() {
            assert_eq!(registry().get(name).unwrap().name(), name);
        }
    }

    #[test]
    fn names_are_sorted_and_unique() {
        let names: Vec<&str> = registry().names().collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(names, sorted);
    }
}
