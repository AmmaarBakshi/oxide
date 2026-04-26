use crate::token::Token;
use std::iter::Peekable;
use std::str::Chars;

pub struct Lexer<'a> {
    input: Peekable<Chars<'a>>,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            input: input.chars().peekable(),
        }
    }

    /// Consumes the input string and turns it into a list of Tokens
    pub fn tokenize(&mut self) -> Vec<Token> {
        let mut tokens = Vec::new();

        while let Some(&c) = self.input.peek() {
            match c {
                
                ' ' | '\t' | '\n' | '\r' => {
                    self.input.next();
                }
                
                '"' => tokens.push(self.lex_string()),
                
                '>' => {
                    self.input.next();
                    tokens.push(Token::RedirectOut);
                }

                '|' => {
                    self.input.next(); // Consume first character
                    if let Some(&'|') = self.input.peek() {
                        self.input.next(); // Consume second '|'
                        tokens.push(Token::Or);
                    } else {
                        tokens.push(Token::Pipe);
                    }
                }
  
                '&' => {
                    self.input.next();
                    if let Some(&'&') = self.input.peek() {
                        self.input.next();
                        tokens.push(Token::And);
                    } else {
                        // If it's just a single '&', treat it as a normal word for now
                        tokens.push(Token::Word("&".to_string()));
                    }
                }
                
                _ => tokens.push(self.lex_word()),
            }
        }

        tokens
    }

    fn lex_word(&mut self) -> Token {
        let mut word = String::new();
        while let Some(&c) = self.input.peek() {
            // Stop building the word if we hit a space or a quote
            if c.is_whitespace() || c == '"' { 
                break;
            }
            word.push(c); 
            self.input.next();
        }
        Token::Word(word)
    }

    fn lex_string(&mut self) -> Token {
        self.input.next(); // Consume the opening quote '"'
        
        let mut string = String::new();
        while let Some(&c) = self.input.peek() {
            self.input.next(); // Consume the character
            if c == '"' {
                break; // End of string!
            }
            string.push(c);
        }
        
        Token::StringLiteral(string)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_lexing() {
        let input = r#"echo "hello world" test"#;
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize();

        assert_eq!(
            tokens,
            vec![
                Token::Word("echo".to_string()),
                Token::StringLiteral("hello world".to_string()),
                Token::Word("test".to_string()),
            ]
        );
    }
}