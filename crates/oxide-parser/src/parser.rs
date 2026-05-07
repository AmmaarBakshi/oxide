use crate::token::Token;
use crate::ast::{Command, Statement, Condition, Executable};

pub struct Parser {
    tokens: Vec<Token>,
    cursor: usize,
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

    fn parse_statement(&mut self) -> Option<Statement> {
        if self.cursor >= self.tokens.len() { return None; }

        // Intercept 'if' keyword
        if let Token::Word(w) = &self.tokens[self.cursor] {
            if w == "if" {
                return self.parse_if_statement();
            }
            if w == "while" {
                return self.parse_while_statement();
            }
            if w == "for" {
                return self.parse_for_statement();
            }
        }

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
                // Stop parsing statement if we hit logic operators or block closers
                Token::And | Token::Or | Token::RBrace | Token::Newline | Token::Semicolon => break,
                _ => self.cursor += 1,
            }
        }

        if let Some(cmd) = current_cmd {
            pipeline.push(cmd);
        }

        if pipeline.is_empty() {
            None
        } else if pipeline.len() == 1 {
            Some(Statement::Command(pipeline.pop().unwrap()))
        } else {
            Some(Statement::Pipeline(pipeline))
        }
    }

    // New Helper: Extracts condition string between keywords (if/elif) and '{'
    fn parse_condition(&mut self) -> String {
        let mut condition = String::new();
        while self.cursor < self.tokens.len() && !matches!(self.tokens[self.cursor], Token::LBrace) {
            condition.push_str(&self.tokens[self.cursor].to_string());
            condition.push(' ');
            self.cursor += 1;
        }
        condition.trim().to_string()
    }

    fn parse_if_statement(&mut self) -> Option<Statement> {
        self.cursor += 1; // consume 'if'
        
        let condition = self.parse_condition();
        let body = self.parse_block();
        
        let mut else_if = Vec::new();
        let mut else_body = None;

        while self.cursor < self.tokens.len() {
            match &self.tokens[self.cursor] {
                Token::Word(w) if w == "elif" => {
                    self.cursor += 1;
                    let elif_cond = self.parse_condition();
                    let elif_body = self.parse_block();
                    else_if.push((elif_cond, elif_body));
                }
                Token::Word(w) if w == "else" => {
                    self.cursor += 1;
                    else_body = Some(self.parse_block());
                    break; // 'else' is the terminal point
                }
                _ => break,
            }
        }

        Some(Statement::If { 
            condition, 
            body, 
            else_if, 
            else_body 
        })
    }

    fn parse_while_statement(&mut self) -> Option<Statement> {
        self.cursor += 1; // consume 'while'
        let condition = self.parse_condition(); // reuse your existing helper!
        let body = self.parse_block();
        
        Some(Statement::While { condition, body })
    }

    fn parse_for_statement(&mut self) -> Option<Statement> {
        self.cursor += 1; // consume 'for'
        
        let mut variable = String::new();
        if let Token::Word(w) = &self.tokens[self.cursor] {
            variable = w.clone();
            self.cursor += 1;
        }

        // consume 'in' if present
        if self.cursor < self.tokens.len() && matches!(self.tokens[self.cursor], Token::Word(ref w) if w == "in") {
            self.cursor += 1;
        }

        // Gather list of items until '{'
        let mut values = Vec::new();
        while self.cursor < self.tokens.len() && !matches!(self.tokens[self.cursor], Token::LBrace) {
            values.push(self.tokens[self.cursor].to_string());
            self.cursor += 1;
        }

        let body = self.parse_block();
        Some(Statement::For { variable, values, body })
    }

    fn parse_block(&mut self) -> Vec<Statement> {
        let mut block = Vec::new();
        
        // Check for '{'
        if self.cursor < self.tokens.len() && matches!(self.tokens[self.cursor], Token::LBrace) {
            self.cursor += 1; // consume '{'

            while self.cursor < self.tokens.len() {
                // Check for '}'
                if matches!(self.tokens[self.cursor], Token::RBrace) {
                    self.cursor += 1; // consume '}'
                    return block;
                }

                if let Some(stmt) = self.parse_statement() {
                    block.push(stmt);
                } else {
                    // Prevent hang if statement fails
                    self.cursor += 1;
                }
            }
        }
        block
    }
}