use crate::ast::{Command, Statement, Condition, Executable}; 
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

    pub fn parse(&mut self) -> Vec<Executable> {
        let mut executables = Vec::new();
        let mut next_condition = Condition::Always;

        while self.tokens.peek().is_some() {
            if let Some(stmt) = self.parse_pipeline() {
                executables.push(Executable {
                    statement: stmt,
                    condition: next_condition.clone(),
                });
            }

            // Check what operator links this command to the next one
            if let Some(token) = self.tokens.peek() {
                match token {
                    Token::And => {
                        next_condition = Condition::And;
                        self.tokens.next(); // Consume '&&'
                    }
                    Token::Or => {
                        next_condition = Condition::Or;
                        self.tokens.next(); // Consume '||'
                    }
                    _ => break, // Unrecognized separator, stop parsing
                }
            } else {
                break;
            }
        }

        executables
    }

    fn parse_pipeline(&mut self) -> Option<Statement> {
        let mut commands = Vec::new();

        while let Some(cmd) = self.parse_command() {
            commands.push(cmd);

            // If the next token is a pipe, consume it and loop again!
            if let Some(token) = self.tokens.peek() {
                if *token == Token::Pipe {
                    self.tokens.next(); // Consume the '|'
                    continue; 
                }
            }
            break; // No pipe? End of the statement.
        }

        if commands.is_empty() {
            None
        } else if commands.len() == 1 {
            Some(Statement::SimpleCommand(commands.remove(0)))
        } else {
            Some(Statement::Pipeline(commands))
        }
    }

    // Notice this now returns Option<Command> instead of Option<Statement>
    fn parse_command(&mut self) -> Option<Command> {
        let program: String;
        let mut args = Vec::new();
        let mut outfile = None;

        if let Some(token) = self.tokens.next() {
            match token {
                Token::Word(w) | Token::StringLiteral(w) => program = w,
                _ => return None,
            }
        } else {
            return None;
        }

        while let Some(token) = self.tokens.peek() {
            match token {
                Token::Word(w) | Token::StringLiteral(w) => {
                    args.push(w.clone());
                    self.tokens.next();
                }
                Token::RedirectOut => {
                    self.tokens.next();
                    if let Some(next_token) = self.tokens.next() {
                        match next_token {
                            Token::Word(file) | Token::StringLiteral(file) => {
                                outfile = Some(file);
                            }
                            _ => {} 
                        }
                    }
                    break;
                }
                Token::Pipe | Token::And | Token::Or => { // <-- UPDATE THIS LINE
                    break; // STOP looking for arguments if we hit a control operator!
                }
            }
        }

        Some(Command { program, args, outfile })
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