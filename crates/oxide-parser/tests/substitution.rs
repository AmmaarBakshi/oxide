//! Nested and quoted command substitution, from the lexer through the
//! expander.
//!
//! [`EchoShell`] stands in for the executor: it only knows `echo`, but it
//! expands `echo`'s arguments with the real expander, so a substitution
//! nested inside another really is run from inside it.

use oxide_parser::ast::Statement;
use oxide_parser::expand::{expand_string, expand_word, Context};
use oxide_parser::lexer::Lexer;
use oxide_parser::parser::Parser;
use oxide_parser::token::Token;

#[derive(Default)]
struct EchoShell {
    /// Every command it was asked to run, outermost first.
    ran: Vec<String>,
}

impl Context for EchoShell {
    fn lookup(&self, name: &str) -> Option<String> {
        (name == "SPACED").then(|| "a  b".to_string())
    }

    fn substitute(&mut self, command: &str) -> String {
        self.ran.push(command.to_string());
        let words: Vec<String> = Lexer::new(command)
            .tokenize()
            .into_iter()
            .filter_map(|token| match token {
                Token::Word(word) => Some(word),
                _ => None,
            })
            .collect();
        assert_eq!(words.first().map(String::as_str), Some("echo"), "{command}");

        let mut args = Vec::new();
        for word in &words[1..] {
            args.extend(expand_word(word, self));
        }
        format!("{}\n", args.join(" "))
    }
}

/// Lexes `input` as one word and expands it to a single string.
fn expand(input: &str) -> (String, Vec<String>) {
    let tokens = Lexer::new(input).tokenize();
    assert_eq!(tokens.len(), 1, "{input:?} should be one word: {tokens:?}");
    let Token::Word(word) = &tokens[0] else {
        panic!("{input:?} is not a word");
    };
    let mut shell = EchoShell::default();
    let value = expand_string(word, &mut shell);
    (value, shell.ran)
}

#[test]
fn substitutions_nest() {
    let (value, ran) = expand("$(echo $(echo deep))");
    assert_eq!(value, "deep");
    assert_eq!(ran, ["echo $(echo deep)", "echo deep"]);
}

#[test]
fn substitutions_nest_three_deep() {
    let (value, ran) = expand("[$(echo a$(echo b$(echo c)))]");
    assert_eq!(value, "[abc]");
    assert_eq!(ran.len(), 3);
}

#[test]
fn quotes_inside_a_quoted_substitution_are_its_own() {
    let (value, _) = expand("\"$(echo \"a  b\")\"");
    assert_eq!(value, "a  b");
}

#[test]
fn quoted_variables_inside_a_substitution_keep_their_spaces() {
    let (value, _) = expand("$(echo \"$SPACED\")");
    assert_eq!(value, "a  b");
    // Unquoted, the inner echo sees two words and joins them with one space.
    let (value, _) = expand("$(echo $SPACED)");
    assert_eq!(value, "a b");
}

#[test]
fn parentheses_in_quotes_do_not_close_the_substitution() {
    let (value, ran) = expand("$(echo ')(' \")\")");
    assert_eq!(value, ")( )");
    assert_eq!(ran, ["echo ')(' \")\""]);
}

#[test]
fn backticks_and_dollar_parens_mix() {
    let (value, _) = expand("$(echo `echo inner`)");
    assert_eq!(value, "inner");
    let (value, ran) = expand("`echo \\`echo escaped\\``");
    assert_eq!(value, "escaped");
    assert_eq!(ran, ["echo `echo escaped`", "echo escaped"]);
}

#[test]
fn single_quotes_and_backslashes_stop_substitution() {
    let (value, ran) = expand("'$(echo no)'");
    assert_eq!(value, "$(echo no)");
    assert!(ran.is_empty());

    let (value, ran) = expand("\\$(echo)");
    assert_eq!(value, "$(echo)");
    assert!(ran.is_empty());
}

#[test]
fn a_substitution_in_an_assignment_stays_with_its_value() {
    let tokens = Lexer::new("OUT=$(echo a | b; c) run it").tokenize();
    let mut executables = Parser::new(tokens).parse();
    let Statement::Command(cmd) = executables.remove(0).statement else {
        panic!("expected a command");
    };
    assert_eq!(cmd.assignments[0].value, "$(echo a | b; c)");
    assert_eq!(cmd.program, "run");
    assert_eq!(cmd.args, ["it"]);
}
