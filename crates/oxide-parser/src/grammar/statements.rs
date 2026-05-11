use crate::parser::Parser;
use crate::ast::Statement;
use crate::token::Token;

// We are adding methods to the Parser struct from another file!
impl Parser {
    pub(crate) fn parse_if_statement(&mut self) -> Option<Statement> {
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
                    break; 
                }
                _ => break,
            }
        }

        Some(Statement::If { condition, body, else_if, else_body })
    }

    pub(crate) fn parse_while_statement(&mut self) -> Option<Statement> {
        self.cursor += 1; // consume 'while'
        let condition = self.parse_condition(); 
        let body = self.parse_block();
        
        Some(Statement::While { condition, body })
    }

    pub(crate) fn parse_for_statement(&mut self) -> Option<Statement> {
        self.cursor += 1; // consume 'for'
        
        let mut variable = String::new();
        if let Token::Word(w) = &self.tokens[self.cursor] {
            variable = w.clone();
            self.cursor += 1;
        }

        // consume 'in'
        if self.cursor < self.tokens.len() && matches!(self.tokens[self.cursor], Token::Word(ref w) if w == "in") {
            self.cursor += 1;
        }

        let mut values = Vec::new();
        while self.cursor < self.tokens.len() && !matches!(self.tokens[self.cursor], Token::LBrace) {
            values.push(self.tokens[self.cursor].to_string());
            self.cursor += 1;
        }

        let body = self.parse_block();
        Some(Statement::For { variable, values, body })
    }

    pub(crate) fn parse_block(&mut self) -> Vec<Statement> {
        let mut block = Vec::new();
        
        if self.cursor < self.tokens.len() && matches!(self.tokens[self.cursor], Token::LBrace) {
            self.cursor += 1; // consume '{'

            while self.cursor < self.tokens.len() {
                if matches!(self.tokens[self.cursor], Token::RBrace) {
                    self.cursor += 1; // consume '}'
                    return block;
                }

                // Call the main parse_statement from parser.rs
                if let Some(stmt) = self.parse_statement() {
                    block.push(stmt);
                } else {
                    self.cursor += 1;
                }
            }
        }
        block
    }
}