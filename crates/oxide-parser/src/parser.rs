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
            let start_pos = self.cursor; // Record current position

            if let Some(statement) = self.parse_statement() {
                executables.push(Executable {
                    statement,
                    condition: current_condition.clone(),
                });
            }

            // --- THE SAFETY VALVE ---
            // If parse_statement didn't move the cursor, we force it forward
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

        // Intercept 'if' keyword for logic blocks
        if let Token::Word(w) = &self.tokens[self.cursor] {
            if w == "if" {
                return self.parse_if_statement();
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
                                cmd.outfile = Some(file.clone()); // Uses the outfile field
                            }
                            self.cursor += 1;
                        }
                    }
                }
                Token::And | Token::Or | Token::RBrace => break,
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

    fn parse_if_statement(&mut self) -> Option<Statement> {
        self.cursor += 1; // consume 'if'
        
        let mut condition = String::new();
        if self.cursor < self.tokens.len() {
            if let Token::Word(w) = &self.tokens[self.cursor] {
                condition = w.clone();
                self.cursor += 1;
            }
        }

        let then_branch = self.parse_block();
        let mut else_branch = None;

        if self.cursor < self.tokens.len() {
            if let Token::Word(w) = &self.tokens[self.cursor] {
                if w == "else" {
                    self.cursor += 1;
                    else_branch = Some(self.parse_block());
                }
            }
        }

        Some(Statement::If { condition, then_branch, else_branch })
    }

    fn parse_block(&mut self) -> Vec<Statement> {
        let mut block = Vec::new();
        if self.cursor < self.tokens.len() && matches!(self.tokens[self.cursor], Token::LBrace) {
            // Inside parse_statement match for Token::If
            let body = self.parse_block();
            let mut else_if = Vec::new();
            let mut else_body = None;

            while self.cursor < self.tokens.len() {
                match &self.tokens[self.cursor] {
                    Token::Word(w) if w == "elif" => {
                        self.cursor += 1;
                        let cond = self.parse_condition(); // helper to get string
                        else_if.push((cond, self.parse_block()));
                    }
                    Token::Word(w) if w == "else" => {
                        self.cursor += 1;
                        else_body = Some(self.parse_block());
                        break; // 'else' is always last
                    }
                    _ => break,
                }
            }
            return Some(Statement::If { condition, body, else_if, else_body });
        }
        block
    }
}