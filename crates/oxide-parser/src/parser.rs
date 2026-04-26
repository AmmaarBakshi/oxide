use crate::ast::{Command, Statement};
use crate::token::Token;
use std::iter::Peekable;
use std::vec::IntoIter;

pub struct Parser {
    tokens: Peekable<IntoIter<Token>>,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens: tokens.into_iter().peekable(),
        }
    }

    pub fn parse(&mut self) -> Vec<Statement> {
        let mut statements = Vec::new();

        // Keep parsing commands until we run out of tokens
        while self.tokens.peek().is_some() {
            if let Some(stmt) = self.parse_command() {
                statements.push(stmt);
            }
        }

        statements
    }

    fn parse_command(&mut self) -> Option<Statement> {
        let program: String;
        let mut args = Vec::new();
        let mut outfile = None; // <-- Track the output file

        if let Some(token) = self.tokens.next() {
            match token {
                Token::Word(w) | Token::StringLiteral(w) => program = w,
                _ => return None, // If a command starts with '>', it's invalid
            }
        } else {
            return None;
        }

        while let Some(token) = self.tokens.peek() {
            match token {
                Token::Word(w) | Token::StringLiteral(w) => {
                    args.push(w.clone());
                    self.tokens.next(); // Consume it
                }
                Token::RedirectOut => {
                    self.tokens.next(); // Consume the '>'
                    
                    // The very next token MUST be the file name
                    if let Some(next_token) = self.tokens.next() {
                        match next_token {
                            Token::Word(file) | Token::StringLiteral(file) => {
                                outfile = Some(file);
                            }
                            _ => {} // Handle errors later
                        }
                    }
                    break; // End the command after redirection
                }
            }
        }

        Some(Statement::SimpleCommand(Command { program, args, outfile }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;

    #[test]
    fn test_parse_simple_command() {
        // 1. Lex the raw text
        let mut lexer = Lexer::new(r#"echo "hello world" test"#);
        let tokens = lexer.tokenize();

        // 2. Parse the tokens into an AST
        let mut parser = Parser::new(tokens);
        let ast = parser.parse();

        // 3. Verify it grouped them correctly!
        assert_eq!(ast.len(), 1);
        assert_eq!(
            ast[0],
            Statement::SimpleCommand(Command {
                program: "echo".to_string(),
                args: vec!["hello world".to_string(), "test".to_string()],
            })
        );
    }
}