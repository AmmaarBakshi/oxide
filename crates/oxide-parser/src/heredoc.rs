//! Here-documents: `cmd <<EOF`, then lines of input up to a line `EOF`.
//!
//! The lexer calls [`collect_body`] once it reaches the end of the line
//! holding the `<<`, so the body arrives as part of the command instead of
//! being run as commands of its own. At the prompt, a body that hasn't been
//! typed yet is read like any other unfinished statement.

/// The word after `<<` that ends a here-document, and how to read the body.
#[derive(Clone, Debug, PartialEq)]
pub struct Delimiter {
    /// The terminating line, with any quotes removed.
    pub word: String,
    /// False when the word was quoted (`<<'EOF'`): the body is then taken
    /// literally, with no `$` expansion.
    pub expand: bool,
    /// `<<-`: leading tabs are stripped from each body line (and from the
    /// terminating line), so the document can be indented with the script.
    pub strip_tabs: bool,
}

impl Delimiter {
    /// Reads the delimiter as written after `<<` or `<<-`.
    pub fn parse(raw: &str, strip_tabs: bool) -> Self {
        let quoting = |c: char| matches!(c, '\'' | '"' | '\\');
        Self {
            word: raw.chars().filter(|&c| !quoting(c)).collect(),
            expand: !raw.contains(quoting),
            strip_tabs,
        }
    }
}

/// Collects a here-document body from `lines`, up to the line that is
/// exactly the delimiter. Returns the body and whether the delimiter was
/// found before the lines ran out.
pub fn collect_body(lines: impl Iterator<Item = String>, delimiter: &Delimiter) -> (String, bool) {
    let mut body = String::new();
    for line in lines {
        let line = line.strip_suffix('\r').unwrap_or(&line);
        let line = if delimiter.strip_tabs {
            line.trim_start_matches('\t')
        } else {
            line
        };
        if line == delimiter.word {
            return (body, true);
        }
        body.push_str(line);
        body.push('\n');
    }
    (body, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(text: &str) -> impl Iterator<Item = String> + '_ {
        text.lines().map(str::to_string)
    }

    #[test]
    fn quoting_the_delimiter_turns_expansion_off() {
        assert_eq!(
            Delimiter::parse("EOF", false),
            Delimiter {
                word: "EOF".into(),
                expand: true,
                strip_tabs: false
            }
        );
        assert!(!Delimiter::parse("'EOF'", false).expand);
        assert!(!Delimiter::parse("\"END\"", true).expand);
        assert_eq!(Delimiter::parse("\\EOF", false).word, "EOF");
    }

    #[test]
    fn the_body_ends_at_the_delimiter_line() {
        let delimiter = Delimiter::parse("EOF", false);
        let (body, found) = collect_body(lines("one\n  EOF\ntwo\nEOF\nafter"), &delimiter);
        assert_eq!(body, "one\n  EOF\ntwo\n");
        assert!(found);
    }

    #[test]
    fn dash_strips_leading_tabs() {
        let delimiter = Delimiter::parse("EOF", true);
        let (body, found) = collect_body(lines("\tindented\n\t\tdeeper\n\tEOF"), &delimiter);
        assert_eq!(body, "indented\ndeeper\n");
        assert!(found);
    }

    #[test]
    fn a_missing_delimiter_is_reported() {
        let delimiter = Delimiter::parse("EOF", false);
        let (body, found) = collect_body(lines("only\nlines"), &delimiter);
        assert_eq!(body, "only\nlines\n");
        assert!(!found);
    }
}
