use crate::parser::Parser;
use crate::ast::{Command, Statement};
use crate::token::Token;

impl Parser {
    /// Parses standard commands, arguments, pipes (|), and redirects (>)
    pub(crate) fn parse_pipeline_or_command(&mut self) -> Option<Statement> {
        let mut pipeline = Vec::new();
        let mut current_cmd: Option<Command> = None;

        while self.cursor < self.tokens.len() {
            match &self.tokens[self.cursor] {
                Token::Word(w) | Token::StringLiteral(w) => {
                    if let Some(cmd) = &mut current_cmd {
                        cmd.args.push(w.clone());
                    } else {
                        current_cmd = Some(Command::new(w.clone()));
                    }
                    self.cursor += 1;
                }
                Token::Pipe => {
                    if let Some(cmd) = current_cmd.take() {
                        pipeline.push(cmd);
                    }
                    self.cursor += 1;
                }
                Token::RedirectOut => {
                    self.cursor += 1;
                    if self.cursor < self.tokens.len() {
                        if let Token::Word(file) | Token::StringLiteral(file) = &self.tokens[self.cursor] {
                            if let Some(cmd) = &mut current_cmd {
                                cmd.outfile = Some(file.clone());
                            }
                            self.cursor += 1;
                        }
                    }
                }
                // Stop parsing this pipeline if we hit logic operators or block closers
                Token::And | Token::Or | Token::RBrace | Token::Newline => break,
                _ => self.cursor += 1,
            }
        }

        // Push the last command into the pipeline list
        if let Some(cmd) = current_cmd {
            pipeline.push(cmd);
        }

        if pipeline.is_empty() {
            None
        } else if pipeline.len() == 1 {
            // It's just a single command (e.g., `ls -la`)
            Some(Statement::Command(pipeline.pop().unwrap()))
        } else {
            // It's a chained pipeline (e.g., `ls -la | grep src`)
            Some(Statement::Pipeline(pipeline))
        }
    }
}