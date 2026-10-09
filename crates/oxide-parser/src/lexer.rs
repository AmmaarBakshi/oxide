use crate::token::Token;
use std::iter::Peekable;
use std::str::Chars;

pub struct Lexer<'a> {
    input: Peekable<Chars<'a>>,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Lexer {
            input: input.chars().peekable(),
        }
    }

    pub fn tokenize(&mut self) -> Vec<Token> {
        let mut tokens = Vec::new();

        while let Some(&c) = self.input.peek() {
            match c {
                // Skip horizontal whitespace, but handle newlines
                ' ' | '\t' | '\r' => {
                    self.input.next();
                }
                '\n' => {
                    self.input.next();
                    tokens.push(Token::Newline);
                }
                // Handle Pipes and OR (|, ||)
                '|' => {
                    self.input.next(); // consume the first |
                    if self.input.peek() == Some(&'|') {
                        self.input.next(); // consume the second |
                        tokens.push(Token::Or);
                    } else {
                        tokens.push(Token::Pipe);
                    }
                }
                // Handle Background and AND (&, &&)
                '&' => {
                    self.input.next();
                    if self.input.peek() == Some(&'&') {
                        self.input.next();
                        tokens.push(Token::And);
                    } else {
                        tokens.push(Token::Background);
                    }
                }
                // Handle Redirects (<, >)
                '>' => {
                    self.input.next(); // consume the first >
                    if self.input.peek() == Some(&'>') {
                        self.input.next(); // consume the second >
                        tokens.push(Token::RedirectAppend);
                    } else {
                        tokens.push(Token::RedirectOut);
                    }
                }
                '<' => {
                    self.input.next();
                    tokens.push(Token::RedirectIn);
                }
                ';' => {
                    self.input.next();
                    tokens.push(Token::Newline);
                }
                '{' => {
                    self.input.next();
                    tokens.push(Token::LBrace);
                }
                '}' => {
                    self.input.next();
                    tokens.push(Token::RBrace);
                }
                // Everything else starts a word: a command, flag or path.
                _ => {
                    let word = self.word();
                    if word == "2" && self.input.peek() == Some(&'>') {
                        // `2>` and `2>>` redirect stderr. Only a bare 2 right
                        // against the `>` counts: `echo 2 > f` writes "2".
                        self.input.next();
                        if self.input.peek() == Some(&'>') {
                            self.input.next();
                            tokens.push(Token::RedirectErrAppend);
                        } else {
                            tokens.push(Token::RedirectErr);
                        }
                    } else if !word.is_empty() {
                        // (A backslash-newline on its own reads as nothing.)
                        tokens.push(Token::Word(word));
                    }
                }
            }
        }
        tokens
    }

    /// Reads one word, keeping its quotes and backslashes as written. They
    /// are interpreted later, when the word is expanded; keeping them is what
    /// lets `NAME="two words"` stay a single word and `'$HOME'` stay literal.
    fn word(&mut self) -> String {
        let mut word = String::new();
        while let Some(&c) = self.input.peek() {
            match c {
                '\'' | '"' => self.quoted(c, &mut word),
                '\\' => {
                    self.input.next();
                    match self.input.next() {
                        // A backslash-newline joins two lines into one.
                        Some('\n') => {}
                        Some(next) => {
                            word.push('\\');
                            word.push(next);
                        }
                        None => word.push('\\'),
                    }
                }
                // Keep a `${...}` expansion intact even though `{`/`}` are
                // otherwise block delimiters — it belongs to this word.
                '$' => {
                    word.push(c);
                    self.input.next();
                    if self.input.peek() == Some(&'{') {
                        for b in self.input.by_ref() {
                            word.push(b);
                            if b == '}' {
                                break;
                            }
                        }
                    }
                }
                c if c.is_whitespace() || "><|&;{}".contains(c) => break,
                _ => {
                    word.push(c);
                    self.input.next();
                }
            }
        }
        word
    }

    /// Copies a quoted section onto `word`, opening and closing quotes
    /// included. An unterminated quote runs to the end of the input.
    fn quoted(&mut self, quote: char, word: &mut String) {
        word.push(quote);
        self.input.next();
        while let Some(c) = self.input.next() {
            word.push(c);
            if c == quote {
                break;
            }
            // Inside double quotes a backslash can escape the closing quote.
            if c == '\\' && quote == '"' {
                if let Some(next) = self.input.next() {
                    word.push(next);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(input: &str) -> Vec<Token> {
        Lexer::new(input).tokenize()
    }

    fn word(text: &str) -> Token {
        Token::Word(text.to_string())
    }

    #[test]
    fn quotes_stay_inside_the_word() {
        assert_eq!(
            words("echo \"a b\"c 'd e'"),
            [word("echo"), word("\"a b\"c"), word("'d e'")]
        );
        assert_eq!(words("NAME=\"two words\""), [word("NAME=\"two words\"")]);
    }

    #[test]
    fn operators_inside_quotes_are_text() {
        assert_eq!(words("echo 'a | b; c'"), [word("echo"), word("'a | b; c'")]);
    }

    #[test]
    fn backslash_keeps_a_separator_in_the_word() {
        assert_eq!(words("touch a\\ b"), [word("touch"), word("a\\ b")]);
    }

    #[test]
    fn two_right_against_an_arrow_redirects_stderr() {
        assert_eq!(
            words("cmd 2>err 2>>log"),
            [
                word("cmd"),
                Token::RedirectErr,
                word("err"),
                Token::RedirectErrAppend,
                word("log")
            ]
        );
        // With a space, or as part of a longer word, it is just text.
        assert_eq!(
            words("echo 2 >f x2>g"),
            [
                word("echo"),
                word("2"),
                Token::RedirectOut,
                word("f"),
                word("x2"),
                Token::RedirectOut,
                word("g")
            ]
        );
    }

    #[test]
    fn backslash_newline_joins_lines() {
        assert_eq!(
            words("echo a \\\n  b"),
            [word("echo"), word("a"), word("b")]
        );
    }
}
