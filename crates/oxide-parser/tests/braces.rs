//! Brace expansion edge cases, through the lexer and the full expander.

use oxide_parser::braces;
use oxide_parser::expand::{expand_string, expand_word, Context};
use oxide_parser::lexer::Lexer;
use oxide_parser::token::Token;

/// Just two variables, and substitutions that echo their command back.
struct Vars;

impl Context for Vars {
    fn lookup(&self, name: &str) -> Option<String> {
        match name {
            "X" => Some("x".into()),
            "SPACED" => Some("a b".into()),
            _ => None,
        }
    }

    fn substitute(&mut self, command: &str) -> String {
        format!("[{}]", command)
    }
}

fn words(input: &str) -> Vec<String> {
    let mut out = Vec::new();
    for token in Lexer::new(input).tokenize() {
        if let Token::Word(word) = token {
            out.extend(expand_word(&word, &mut Vars));
        }
    }
    out
}

#[test]
fn adjacent_and_repeated_pairs() {
    assert_eq!(words("{a,b}{a,b}"), ["aa", "ab", "ba", "bb"]);
    assert_eq!(words("x{a,b}y{1,2}z").len(), 4);
}

#[test]
fn deep_nesting_keeps_order() {
    assert_eq!(words("{a,{b,{c,d}}}"), ["a", "b", "c", "d"]);
    assert_eq!(words("{a{1,2},b}"), ["a1", "a2", "b"]);
}

#[test]
fn a_range_and_a_list_combine() {
    assert_eq!(words("{a,b}{1..2}"), ["a1", "a2", "b1", "b2"]);
}

#[test]
fn single_items_empties_and_lone_braces_stay_as_written() {
    assert_eq!(words("{x}"), ["{x}"]);
    assert_eq!(words("{}"), ["{}"]);
    // Two empty words, which vanish like any other empty unquoted word.
    assert_eq!(words("{,}"), Vec::<String>::new());
    assert_eq!(words("a{,}b"), ["ab", "ab"]);
    assert_eq!(
        words("}{"),
        Vec::<String>::new(),
        "a bare `}}` is a block closer"
    );
}

#[test]
fn unbalanced_braces_stay_literal() {
    assert_eq!(words("{a,b"), ["{a,b"]);
    assert_eq!(words("a,b}"), ["a,b"]);
}

#[test]
fn variables_expand_after_the_braces() {
    assert_eq!(words("{$X,y}"), ["x", "y"]);
    // A variable's value is not brace-expanded itself.
    assert_eq!(braces::expand("$X"), ["$X"]);
    // Unquoted, a value with a space still splits afterwards.
    assert_eq!(words("{$SPACED,c}"), ["a", "b", "c"]);
}

#[test]
fn quoting_turns_braces_off_part_way() {
    assert_eq!(words("'{a,b}'{c,d}"), ["{a,b}c", "{a,b}d"]);
    assert_eq!(words("\"{\"a,b'}'"), ["{a,b}"]);
    // A `}` only stays in a word whose own unquoted `{` it closes; any
    // other ends the word, so `{ echo hi}` still closes its block.
    assert_eq!(words("\"{\"a,b}"), ["{a,b"]);
}

#[test]
fn substitutions_are_run_once_per_alternative() {
    assert_eq!(words("{a,b}$(cmd)"), ["a[cmd]", "b[cmd]"]);
}

#[test]
fn single_string_expansion_skips_braces() {
    // As for assignments: `X={a,b}` keeps its braces.
    assert_eq!(expand_string("{a,b}", &mut Vars), "{a,b}");
}

#[test]
fn ranges_are_inclusive_even_when_the_step_overshoots() {
    assert_eq!(words("{1..4..3}"), ["1", "4"]);
    assert_eq!(words("{1..5..3}"), ["1", "4"]);
    assert_eq!(words("{5..5}"), ["5"]);
}
