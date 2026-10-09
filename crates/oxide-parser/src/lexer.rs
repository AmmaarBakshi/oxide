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
                // Handle Background and AND (&, &&), and &> / &>> which
                // send both stdout and stderr to a file.
                '&' => {
                    let token = if self.eat("&&") {
                        Token::And
                    } else if self.eat("&>>") {
                        Token::RedirectAllAppend
                    } else if self.eat("&>") {
                        Token::RedirectAll
                    } else {
                        self.input.next();
                        Token::Background
                    };
                    tokens.push(token);
                }
                // Handle Redirects (>, >>, and >&2 for "stdout to stderr")
                '>' => {
                    let token = if self.eat(">&2") {
                        Token::OutToErr
                    } else if self.eat(">>") {
                        Token::RedirectAppend
                    } else {
                        self.input.next();
                        Token::RedirectOut
                    };
                    tokens.push(token);
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
                        // `2>`, `2>>` and `2>&1` redirect stderr. Only a bare
                        // 2 right against the `>` counts: `echo 2 > f` writes "2".
                        let token = if self.eat(">&1") {
                            Token::ErrToOut
                        } else if self.eat(">>") {
                            Token::RedirectErrAppend
                        } else {
                            self.input.next();
                            Token::RedirectErr
                        };
                        tokens.push(token);
                    } else if !word.is_empty() {
                        // (A backslash-newline on its own reads as nothing.)
                        tokens.push(Token::Word(word));
                    }
                }
            }
        }
        tokens
    }

    /// Consumes `s` if the input continues with it, and says whether it did.
    fn eat(&mut self, s: &str) -> bool {
        let mut ahead = self.input.clone();
        for expected in s.chars() {
            if ahead.next() != Some(expected) {
                return false;
            }
        }
        self.input = ahead;
        true
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
    fn stream_joining_operators() {
        assert_eq!(
            words("a 2>&1 >&2 &>f &>>g & b && c"),
            [
                word("a"),
                Token::ErrToOut,
                Token::OutToErr,
                Token::RedirectAll,
                word("f"),
                Token::RedirectAllAppend,
                word("g"),
                Token::Background,
                word("b"),
                Token::And,
                word("c")
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
