//! Brace expansion: `pre{a,b}post` becomes `preapost prebpost`.
//!
//! It works on a word as typed, before any other expansion, and only on
//! braces outside quotes. A pair with no comma at its top level is left as
//! it is, so `{}` and `{x}` stay literal, and so does `${NAME}`.

/// Expands every brace pair in `word`. Several pairs multiply out, left to
/// right, and pairs nested inside an alternative expand in turn.
pub fn expand(word: &str) -> Vec<String> {
    match find(word) {
        None => vec![word.to_string()],
        Some(found) => {
            let prefix = &word[..found.open];
            let suffix = &word[found.close + 1..];
            found
                .parts
                .iter()
                .flat_map(|part| expand(&format!("{}{}{}", prefix, part, suffix)))
                .collect()
        }
    }
}

/// A brace pair that expands: where it is and what it stands for.
struct Found {
    /// Byte offsets of the `{` and its matching `}`.
    open: usize,
    close: usize,
    parts: Vec<String>,
}

/// Finds the first unquoted brace pair in `word` that expands.
fn find(word: &str) -> Option<Found> {
    let chars: Vec<(usize, char)> = word.char_indices().collect();
    let mut i = 0;
    let mut quote = None;
    while i < chars.len() {
        let c = chars[i].1;
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some('"'), '\\') => i += 1,
            (Some(_), _) => {}
            (None, '\'' | '"') => quote = Some(c),
            (None, '\\') => i += 1,
            // `${...}` and `$(...)` are not brace expansion; skip them whole.
            (None, '$') => {
                if let Some(&(_, next @ ('{' | '('))) = chars.get(i + 1) {
                    i = matching(&chars, i + 1, next);
                }
            }
            (None, '{') => {
                if let Some(found) = alternatives(word, &chars, i) {
                    return Some(found);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// The index of the bracket closing the one at `open`, or the last index
/// if it never closes.
fn matching(chars: &[(usize, char)], open: usize, bracket: char) -> usize {
    let close = if bracket == '{' { '}' } else { ')' };
    let mut depth = 0;
    for (i, &(_, c)) in chars.iter().enumerate().skip(open) {
        if c == bracket {
            depth += 1;
        } else if c == close {
            depth -= 1;
            if depth == 0 {
                return i;
            }
        }
    }
    chars.len() - 1
}

/// Reads the brace pair opening at `chars[start]`: `Some` with its
/// comma-separated alternatives, or `None` if it isn't closed or has no
/// comma at its top level.
fn alternatives(word: &str, chars: &[(usize, char)], start: usize) -> Option<Found> {
    let open = chars[start].0;
    let mut depth = 0;
    let mut quote = None;
    let mut commas = Vec::new();
    let mut i = start;
    while i < chars.len() {
        let (pos, c) = chars[i];
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some('"'), '\\') => i += 1,
            (Some(_), _) => {}
            (None, '\'' | '"') => quote = Some(c),
            (None, '\\') => i += 1,
            (None, '{') => depth += 1,
            (None, ',') if depth == 1 => commas.push(pos),
            (None, '}') => {
                depth -= 1;
                if depth == 0 {
                    if commas.is_empty() {
                        return None;
                    }
                    let mut parts = Vec::new();
                    let mut from = open + 1;
                    for comma in commas {
                        parts.push(word[from..comma].to_string());
                        from = comma + 1;
                    }
                    parts.push(word[from..pos].to_string());
                    return Some(Found {
                        open,
                        close: pos,
                        parts,
                    });
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::expand;

    #[test]
    fn alternatives_multiply_with_prefix_and_suffix() {
        assert_eq!(expand("{a,b,c}"), ["a", "b", "c"]);
        assert_eq!(expand("pre{x,y}.rs"), ["prex.rs", "prey.rs"]);
    }

    #[test]
    fn several_and_nested_pairs() {
        assert_eq!(expand("{a,b}{1,2}"), ["a1", "a2", "b1", "b2"]);
        assert_eq!(expand("{a,b{1,2}}"), ["a", "b1", "b2"]);
    }

    #[test]
    fn empty_alternatives_count() {
        assert_eq!(expand("file{,.bak}"), ["file", "file.bak"]);
    }

    #[test]
    fn braces_without_a_comma_stay_literal() {
        assert_eq!(expand("{}"), ["{}"]);
        assert_eq!(expand("{x}"), ["{x}"]);
        assert_eq!(expand("{a,b"), ["{a,b"]);
    }

    #[test]
    fn quoted_and_escaped_braces_stay_literal() {
        assert_eq!(expand("'{a,b}'"), ["'{a,b}'"]);
        assert_eq!(expand("\"{a,b}\""), ["\"{a,b}\""]);
        assert_eq!(expand("\\{a,b}"), ["\\{a,b}"]);
        // A quoted comma doesn't split, but the pair still expands.
        assert_eq!(expand("{a',',b}"), ["a','", "b"]);
    }

    #[test]
    fn parameter_and_command_braces_are_left_alone() {
        assert_eq!(expand("${A,B}"), ["${A,B}"]);
        assert_eq!(expand("$(echo {a,b})"), ["$(echo {a,b})"]);
        assert_eq!(expand("${X}{1,2}"), ["${X}1", "${X}2"]);
    }
}
