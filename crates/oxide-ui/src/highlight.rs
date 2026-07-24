//! Syntax highlighting for the interactive line editor.
//!
//! Two surfaces are highlighted: the prompt (`colorize_prompt`) and the input
//! line the user is typing (`highlight_line`). Colors come from the active
//! [`Theme`]. Both functions return ANSI-wrapped copies of the text —
//! crucially, they never add or remove *visible* characters, so rustyline's
//! cursor math (which runs against the raw line) stays correct.

use std::borrow::Cow;

use oxide_config::theme::RESET;
use oxide_config::Theme;

/// Colorize the leading accent word of the prompt (e.g. "oxide"). We wrap it
/// here rather than baking ANSI into the prompt string so rustyline measures
/// the prompt's *visible* width correctly and the cursor lands in the right
/// place.
pub fn colorize_prompt<'p>(prompt: &'p str, accent_word: &str, theme: &Theme) -> Cow<'p, str> {
    if accent_word.is_empty() || theme.accent.is_empty() {
        return Cow::Borrowed(prompt);
    }
    match prompt.strip_prefix(accent_word) {
        Some(rest) => Cow::Owned(format!("{}{accent_word}{RESET}{rest}", theme.accent)),
        None => Cow::Borrowed(prompt),
    }
}

/// Grey out an autosuggestion hint (fish-style ghost text).
pub fn colorize_hint(hint: &str, theme: &Theme) -> String {
    if theme.hint.is_empty() {
        return hint.to_string();
    }
    format!("{}{hint}{RESET}", theme.hint)
}

fn is_op_char(c: char) -> bool {
    matches!(c, '|' | '>' | '<' | '&' | ';')
}

/// Append `text` wrapped in `color` (or plain, if `color` is empty).
fn push_colored(out: &mut String, color: &str, text: &str) {
    if color.is_empty() {
        out.push_str(text);
    } else {
        out.push_str(color);
        out.push_str(text);
        out.push_str(RESET);
    }
}

/// Highlight a full input line. `commands` is the sorted list of known command
/// names (builtins + executables on PATH) used to distinguish a recognized
/// command from an unknown one; `theme` supplies the colors.
///
/// Coloring rules:
/// - first word of each command (start of line, or after `|`/`;`/`&`):
///   `theme.command` if recognized (or looks like a path), else
///   `theme.unknown_command`;
/// - `-`/`--` flags: `theme.flag`;
/// - quoted strings: `theme.string`;
/// - operators (`|`, `>`, `>>`, `<`, `&`, `&&`, `;`): `theme.operator`;
/// - everything else: default terminal color.
pub fn highlight_line<'l>(line: &'l str, commands: &[String], theme: &Theme) -> Cow<'l, str> {
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
            push_colored(&mut out, &theme.operator, &op);
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
            push_colored(&mut out, &theme.string, &s);
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

        let color: &str = if expect_command {
            // A path to an executable (contains a separator) is assumed valid
            // rather than flagged as an unknown command.
            if word.contains('/') || word.contains('\\') || is_known(commands, &word) {
                &theme.command
            } else {
                &theme.unknown_command
            }
        } else if word.starts_with('-') {
            &theme.flag
        } else {
            ""
        };

        push_colored(&mut out, color, &word);
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

    fn theme() -> Theme {
        Theme::default()
    }

    #[test]
    fn colorizes_leading_accent_word() {
        let t = theme();
        let out = colorize_prompt("oxide C:\\path > ", "oxide", &t);
        assert_eq!(out, format!("{}oxide{RESET} C:\\path > ", t.accent));
        assert!(out.contains("C:\\path > "));
    }

    #[test]
    fn leaves_prompt_without_accent_word_untouched() {
        let out = colorize_prompt("$ ", "oxide", &theme());
        assert_eq!(out, "$ ");
    }

    #[test]
    fn empty_accent_word_disables_prompt_color() {
        let out = colorize_prompt("oxide > ", "", &theme());
        assert_eq!(out, "oxide > ");
    }

    #[test]
    fn empty_line_is_borrowed_unchanged() {
        assert_eq!(highlight_line("", &cmds(), &theme()), "");
    }

    #[test]
    fn known_command_uses_command_color_unknown_uses_unknown() {
        let t = theme();
        let known = highlight_line("ls", &cmds(), &t);
        assert!(known.contains(&t.command) && known.contains("ls"));

        let unknown = highlight_line("frobnicate", &cmds(), &t);
        assert!(unknown.contains(&t.unknown_command) && unknown.contains("frobnicate"));
    }

    #[test]
    fn flags_and_strings_are_colored() {
        let t = theme();
        let out = highlight_line("ls -la \"my file\"", &cmds(), &t);
        assert!(out.contains(&t.flag)); // -la
        assert!(out.contains(&t.string)); // "my file"
    }

    #[test]
    fn command_after_pipe_is_re_evaluated() {
        let t = theme();
        let out = highlight_line("cat x | bogus", &cmds(), &t);
        assert!(out.contains(&t.operator)); // the pipe
        assert!(out.contains(&t.unknown_command)); // bogus flagged as unknown
    }

    #[test]
    fn path_command_is_not_flagged_unknown() {
        let t = theme();
        let out = highlight_line("./tool.exe", &cmds(), &t);
        assert!(out.contains(&t.command));
        assert!(!out.contains(&t.unknown_command));
    }

    #[test]
    fn minimal_theme_leaves_line_plain() {
        let t = oxide_config::theme::named("minimal").unwrap();
        // No command/flag/string colors, so the visible text is unchanged.
        let out = highlight_line("ls -la foo", &cmds(), &t);
        assert_eq!(out, "ls -la foo");
    }

    #[test]
    fn visible_text_is_preserved() {
        let line = "ls -la | cat > out.txt";
        let hl = highlight_line(line, &cmds(), &theme());
        assert_eq!(strip_ansi(&hl), line);
    }

    fn strip_ansi(s: &str) -> String {
        let mut out = String::new();
        let mut chars = s.chars();
        while let Some(c) = chars.next() {
            if c == '\x1b' {
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
