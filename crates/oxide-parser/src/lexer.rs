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
                // Skip whitespace
                ' ' | '\t' | '\n' | '\r' => {
                    self.input.next();
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
                    self.input.next();
                    tokens.push(Token::RedirectOut);
                }
                '<' => {
                    self.input.next();
                    tokens.push(Token::RedirectIn);
                }
                // Handle Quoted Strings (keeps spaces intact!)
                '"' | '\'' => {
                    let quote_type = self.input.next().unwrap(); // consume the opening quote
                    let mut word = String::new();
                    
                    while let Some(&next_c) = self.input.peek() {
                        if next_c == quote_type {
                            self.input.next(); // consume the closing quote
                            break;
                        }
                        word.push(self.input.next().unwrap());
                    }
                    tokens.push(Token::Word(word));
                }
                '{' => tokens.push(Token::LBrace),
                '}' => tokens.push(Token::RBrace),
                // Handle normal words (commands, flags, paths)
                _ => {
                    let mut word = String::new();
                    while let Some(&next_c) = self.input.peek() {
                        if next_c.is_whitespace() || "><|&\"'".contains(next_c) {
                            break;
                        }
                        word.push(self.input.next().unwrap());
                    }
                    tokens.push(Token::Word(word));
                }
            }
        }
        tokens
    }
}