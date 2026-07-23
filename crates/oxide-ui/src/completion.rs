//! Tab-completion.
//!
//! Completes the *first* word of a command against the set of known commands
//! (builtins + executables found on `PATH`), and every other word against the
//! filesystem via rustyline's `FilenameCompleter`.

use std::collections::BTreeSet;

use rustyline::completion::{Completer, FilenameCompleter, Pair};
use rustyline::error::ReadlineError;
use rustyline::Context;

pub struct OxideCompleter {
    files: FilenameCompleter,
    /// Sorted, de-duplicated command names. Kept sorted so `highlight_line`
    /// can binary-search it.
    commands: Vec<String>,
}

impl Default for OxideCompleter {
    fn default() -> Self {
        Self::new()
    }
}

impl OxideCompleter {
    pub fn new() -> Self {
        Self {
            files: FilenameCompleter::new(),
            commands: collect_commands(),
        }
    }

    /// The known-command list, for use by the highlighter.
    pub fn commands(&self) -> &[String] {
        &self.commands
    }
}

impl Completer for OxideCompleter {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        ctx: &Context<'_>,
    ) -> Result<(usize, Vec<Pair>), ReadlineError> {
        let start = word_start(line, pos);
        let word = &line[start..pos];

        if in_command_position(line, start) && !word.contains('/') && !word.contains('\\') {
            let matches: Vec<Pair> = self
                .commands
                .iter()
                .filter(|c| c.starts_with(word))
                .map(|c| Pair {
                    display: c.clone(),
                    replacement: format!("{c} "),
                })
                .collect();
            if !matches.is_empty() {
                return Ok((start, matches));
            }
            // No command matched — fall through to filename completion so
            // things like `./scr<TAB>` still work.
        }

        self.files.complete(line, pos, ctx)
    }
}

/// Byte index where the word under the cursor begins (start of line, or one
/// past the last whitespace before `pos`).
fn word_start(line: &str, pos: usize) -> usize {
    line[..pos]
        .rfind(char::is_whitespace)
        .map(|i| i + 1)
        .unwrap_or(0)
}

/// Whether the word starting at `start` is the command word — i.e. it is the
/// first token, or the first token after a `|`, `;`, or `&`.
fn in_command_position(line: &str, start: usize) -> bool {
    let before = line[..start].trim_end();
    before.is_empty()
        || before.ends_with('|')
        || before.ends_with(';')
        || before.ends_with('&')
}

/// Builtins plus every executable name found on `PATH`, sorted and deduped.
fn collect_commands() -> Vec<String> {
    let mut set: BTreeSet<String> = BTreeSet::new();
    set.insert("exit".to_string());
    for name in oxide_builtins::NAMES {
        set.insert((*name).to_string());
    }
    collect_path_executables(&mut set);
    set.into_iter().collect()
}

#[cfg(windows)]
fn collect_path_executables(set: &mut BTreeSet<String>) {
    // Extensions that make a file directly runnable, per %PATHEXT%.
    let pathext: Vec<String> = std::env::var("PATHEXT")
        .unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_string())
        .split(';')
        .map(|e| e.trim_start_matches('.').to_ascii_lowercase())
        .filter(|e| !e.is_empty())
        .collect();

    let path = match std::env::var_os("PATH") {
        Some(p) => p,
        None => return,
    };
    for dir in std::env::split_paths(&path) {
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_ascii_lowercase());
            if let Some(ext) = ext {
                if pathext.contains(&ext) {
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        set.insert(stem.to_ascii_lowercase());
                    }
                }
            }
        }
    }
}

#[cfg(not(windows))]
fn collect_path_executables(set: &mut BTreeSet<String>) {
    use std::os::unix::fs::PermissionsExt;

    let path = match std::env::var_os("PATH") {
        Some(p) => p,
        None => return,
    };
    for dir in std::env::split_paths(&path) {
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let Ok(meta) = entry.metadata() else { continue };
            // Regular file with any execute bit set.
            if meta.is_file() && meta.permissions().mode() & 0o111 != 0 {
                if let Some(name) = entry.file_name().to_str() {
                    set.insert(name.to_string());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn word_start_finds_last_token() {
        assert_eq!(word_start("ls -la fi", 9), 7);
        assert_eq!(word_start("ls", 2), 0);
        assert_eq!(word_start("ls ", 3), 3);
    }

    #[test]
    fn command_position_detection() {
        assert!(in_command_position("l", 0));
        assert!(in_command_position("ls foo | c", 9));
        assert!(in_command_position("a; b", 3));
        assert!(!in_command_position("ls fi", 3)); // argument, not command
    }

    #[test]
    fn builtins_are_always_present() {
        let cmds = collect_commands();
        assert!(cmds.contains(&"cd".to_string()));
        assert!(cmds.contains(&"exit".to_string()));
        // Result must be sorted for the highlighter's binary search.
        let mut sorted = cmds.clone();
        sorted.sort();
        assert_eq!(cmds, sorted);
    }
}
