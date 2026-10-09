use crate::ast::{List, Statement};
use crate::parser::Parser;
use crate::token::Token;

// We are adding methods to the Parser struct from another file!
impl Parser {
    pub(crate) fn parse_if_statement(&mut self) -> Option<Statement> {
        self.cursor += 1; // consume 'if'

        let condition = self.parse_condition();
        let body = self.parse_block();

        let mut else_if = Vec::new();
        let mut else_body = None;

        while self.continues_with(&["elif", "else"]) {
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

        Some(Statement::If {
            condition,
            body,
            else_if,
            else_body,
        })
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
        if self.cursor < self.tokens.len()
            && matches!(self.tokens[self.cursor], Token::Word(ref w) if w == "in")
        {
            self.cursor += 1;
        }

        let mut values = Vec::new();
        while self.cursor < self.tokens.len() && !matches!(self.tokens[self.cursor], Token::LBrace)
        {
            values.push(self.tokens[self.cursor].to_string());
            self.cursor += 1;
        }

        let body = self.parse_block();
        Some(Statement::For {
            variable,
            values,
            body,
        })
    }

    /// Parses a `{ ... }` body: a list of statements up to the matching `}`.
    pub(crate) fn parse_block(&mut self) -> List {
        if self.tokens.get(self.cursor) != Some(&Token::LBrace) {
            return List::new();
        }
        self.cursor += 1; // consume '{'
        let block = self.parse_list(|token| *token == Token::RBrace);
        if self.tokens.get(self.cursor) == Some(&Token::RBrace) {
            self.cursor += 1; // consume '}'
        }
        block
    }

    /// Whether the next word, past any newlines, is one of `words`. If so
    /// the newlines are consumed, so `}` and `else {` may sit on separate
    /// lines.
    fn continues_with(&mut self, words: &[&str]) -> bool {
        let mut ahead = self.cursor;
        while self.tokens.get(ahead) == Some(&Token::Newline) {
            ahead += 1;
        }
        match self.tokens.get(ahead) {
            Some(Token::Word(w)) if words.contains(&w.as_str()) => {
                self.cursor = ahead;
                true
            }
            _ => false,
        }
    }
}
