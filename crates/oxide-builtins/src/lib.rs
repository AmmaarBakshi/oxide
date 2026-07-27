pub mod alias;
pub mod cd;
pub mod ls;
pub mod pwd;
pub mod export;
pub mod kill;
pub mod sleep;
pub mod rm;
pub mod ps;
pub mod top;
pub mod open;
pub mod find;
pub mod clear;
pub mod touch;
pub mod cat;
pub mod env;
pub mod history;
pub mod help;
pub mod source;
pub mod unset;
pub mod grep;
pub mod bind;
pub mod mkdir;

/// Names of every shell builtin. Consumed by the interactive line editor
/// (`oxide-ui`) for tab-completion and syntax highlighting so the two never
/// drift out of sync with the module list above.
pub const NAMES: &[&str] = &[
    "alias", "cd", "ls", "pwd", "export", "kill", "sleep", "rm", "ps", "top",
    "open", "find", "clear", "touch", "cat", "env", "history", "help", "source",
    "unset", "grep", "bind", "mkdir",
];