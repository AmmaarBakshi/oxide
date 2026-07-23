//! Syntax highlighting for the interactive line editor.
//!
//! Two surfaces are highlighted: the prompt (`colorize_prompt`) and the input
//! line the user is typing (`highlight_line`). Both return ANSI-wrapped copies
//! of the text — crucially, they never add or remove *visible* characters, so
//! rustyline's cursor math (which runs against the raw line) stays correct.

use std::borrow::Cow;

// Rusty-orange (#B7410E), Oxide's accent color.
pub(crate) const ORANGE: &str = "\x1b[38;2;183;65;14m";
pub(crate) const RED: &str = "\x1b[31m";
pub(crate) const GREEN: &str = "\x1b[32m";
pub(crate) const YELLOW: &str = "\x1b[33m";
pub(crate) const CYAN: &str = "\x1b[36m";
pub(crate) const DIM: &str = "\x1b[90m";
pub(crate) const RESET: &str = "\x1b[0m";

/// Colorize the leading "oxide" of the prompt. We wrap it here rather than
/// baking ANSI into the prompt string so rustyline measures the prompt's
/// *visible* width correctly and the cursor lands in the right place.
pub fn colorize_prompt(prompt: &str) -> Cow<'_, str> {
    match prompt.strip_prefix("oxide") {
        Some(rest) => Cow::Owned(format!("{ORANGE}oxide{RESET}{rest}")),
        None => Cow::Borrowed(prompt),
    }
}

/// Grey out an autosuggestion hint (fish-style ghost text).
pub fn colorize_hint(hint: &str) -> String {
    format!("{DIM}{hint}{RESET}")
}

fn is_op_char(c: char) -> bool {
    matches!(c, '|' | '>' | '<' | '&' | ';')
}

fn push_colored(out: &mut String, color: &str, text: &str) {
    out.push_str(color);
    out.push_str(text);
    out.push_str(RESET);
}

/// Highlight a full input line. `commands` is the sorted list of known command
/// names (builtins + executables on PATH) used to distinguish a recognized
/// command from an unknown one.
///
/// Coloring rules:
/// - first word of each command (start of line, or after `|`/`;`/`&`):
///   orange if recognized (or looks like a path), red otherwise;
/// - `-`/`--` flags: yellow;
/// - quoted strings: green;
/// - operators (`|`, `>`, `>>`, `<`, `&`, `&&`, `;`): cyan;
/// - everything else: default terminal color.
pub fn highlight_line<'l>(line: &'l str, commands: &[String]) -> Cow<'l, str> {
    if line.is_empty() {
        return Cow::Borrowed(line);
    }

    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();
    let mut out = String::with_capacity(line.len() + 16);
    let mut i = 0;
    // True when the next word is in command position.
    let mut expect_command = true;

    while i < len {
        let c = chars[i];

        // Whitespace: copy verbatim.
        if c.is_whitespace() {
            out.push(c);
            i += 1;
            continue;
        }

        // Operators: consume the whole run (so `>>` / `&&` / `||` stay together).
        if is_op_char(c) {
            let start = i;
            while i < len && is_op_char(chars[i]) {
                i += 1;
            }
            let op: String = chars[start..i].iter().collect();
            // `|`, `;`, `&` start a new command; `>`/`<` (redirection) do not.
            if op.contains('|') || op.contains(';') || op.contains('&') {
                expect_command = true;
            }
            push_colored(&mut out, CYAN, &op);
            continue;
        }

        // Quoted strings: run to the matching quote (or end of line).
        if c == '"' || c == '\'' {
            let quote = c;
            let start = i;
            i += 1;
            while i < len && chars[i] != quote {
                i += 1;
            }
            if i < len {
                i += 1; // include the closing quote
            }
            let s: String = chars[start..i].iter().collect();
            push_colored(&mut out, GREEN, &s);
            expect_command = false;
            continue;
        }

        // A bare word: read until whitespace, an operator, or a quote.
        let start = i;
        while i < len
            && !chars[i].is_whitespace()
            && !is_op_char(chars[i])
            && chars[i] != '"'
            && chars[i] != '\''
        {
            i += 1;
        }
        let word: String = chars[start..i].iter().collect();

        let color = if expect_command {
            // A path to an executable (contains a separator) is assumed valid
            // rather than flagged as an unknown command.
            if word.contains('/') || word.contains('\\') || is_known(commands, &word) {
                ORANGE
            } else {
                RED
            }
        } else if word.starts_with('-') {
            YELLOW
        } else {
            ""
        };

        if color.is_empty() {
            out.push_str(&word);
        } else {
            push_colored(&mut out, color, &word);
        }
        expect_command = false;
    }

    Cow::Owned(out)
}

fn is_known(commands: &[String], word: &str) -> bool {
    commands.binary_search_by(|c| c.as_str().cmp(word)).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmds() -> Vec<String> {
        // Must stay sorted for the binary_search in `is_known`.
        vec!["cat".into(), "echo".into(), "ls".into()]
    }

    #[test]
    fn colorizes_leading_oxide() {
        let out = colorize_prompt("oxide C:\\path > ");
        assert_eq!(out, format!("{ORANGE}oxide{RESET} C:\\path > "));
        assert!(out.starts_with(ORANGE));
        assert!(out.contains("C:\\path > "));
    }

    #[test]
    fn leaves_unrelated_prompt_untouched() {
        let out = colorize_prompt("$ ");
        assert_eq!(out, "$ ");
    }

    #[test]
    fn empty_line_is_borrowed_unchanged() {
        assert_eq!(highlight_line("", &cmds()), "");
    }

    #[test]
    fn known_command_is_orange_unknown_is_red() {
        let known = highlight_line("ls", &cmds());
        assert!(known.contains(ORANGE) && known.contains("ls"));
        assert!(!known.contains(RED));

        let unknown = highlight_line("frobnicate", &cmds());
        assert!(unknown.contains(RED) && unknown.contains("frobnicate"));
    }

    #[test]
    fn flags_are_yellow_and_strings_green() {
        let out = highlight_line("ls -la \"my file\"", &cmds());
        assert!(out.contains(YELLOW)); // -la
        assert!(out.contains(GREEN)); // "my file"
    }

    #[test]
    fn command_after_pipe_is_re_evaluated() {
        // `cat` (known) then `bogus` (unknown) after the pipe.
        let out = highlight_line("cat x | bogus", &cmds());
        assert!(out.contains(CYAN)); // the pipe
        assert!(out.contains(RED)); // bogus flagged as unknown command
    }

    #[test]
    fn path_command_is_not_flagged_red() {
        let out = highlight_line("./tool.exe", &cmds());
        assert!(out.contains(ORANGE));
        assert!(!out.contains(RED));
    }

    #[test]
    fn visible_text_is_preserved() {
        // Stripping ANSI must return the original line exactly.
        let line = "ls -la | cat > out.txt";
        let hl = highlight_line(line, &cmds());
        let stripped = strip_ansi(&hl);
        assert_eq!(stripped, line);
    }

    fn strip_ansi(s: &str) -> String {
        let mut out = String::new();
        let mut chars = s.chars();
        while let Some(c) = chars.next() {
            if c == '\x1b' {
                // Skip until the terminating 'm' of the SGR sequence.
                for c in chars.by_ref() {
                    if c == 'm' {
                        break;
                    }
                }
            } else {
                out.push(c);
            }
        }
        out
    }
}
