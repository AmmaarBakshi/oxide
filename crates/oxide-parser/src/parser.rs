use crate::token::Token;
use crate::ast::{Statement, Condition, Executable};

pub struct Parser {
    pub(crate) tokens: Vec<Token>,
    pub(crate) cursor: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, cursor: 0 }
    }

    pub fn parse(&mut self) -> Vec<Executable> {
        let mut executables = Vec::new();
        let mut current_condition = Condition::Always;

        while self.cursor < self.tokens.len() {
            // Skip separators
            while self.cursor < self.tokens.len() && matches!(self.tokens[self.cursor], Token::Newline | Token::Semicolon) {
                self.cursor += 1;
            }
            if self.cursor >= self.tokens.len() {
                break;
            }

            let start_pos = self.cursor;

            if let Some(statement) = self.parse_statement() {
                executables.push(Executable {
                    statement,
                    condition: current_condition.clone(),
                });
            }

            // Safety valve to prevent infinite loops
            if self.cursor == start_pos {
                self.cursor += 1;
            }

            if self.cursor < self.tokens.len() {
                match &self.tokens[self.cursor] {
                    Token::And => {
                        current_condition = Condition::And;
                        self.cursor += 1;
                    }
                    Token::Or => {
                        current_condition = Condition::Or;
                        self.cursor += 1;
                    }
                    _ => {
                        current_condition = Condition::Always;
                    }
                }
            }
        }
        executables
    }

    pub(crate) fn parse_statement(&mut self) -> Option<Statement> {
        if self.cursor >= self.tokens.len() { return None; }

        // 1. Check if the statement starts with a logic keyword
        if let Token::Word(w) = &self.tokens[self.cursor] {
            match w.as_str() {
                "if" => return self.parse_if_statement(),
                "while" => return self.parse_while_statement(),
                "for" => return self.parse_for_statement(),
                _ => {} // Fall through to pipeline parsing
            }
        }

        // 2. If it's not a logic block, it must be a standard command/pipeline
        self.parse_pipeline_or_command()
    }
}