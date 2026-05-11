use crate::parser::Parser;
use crate::token::Token;

impl Parser {
    // Extracts condition strings between keywords (if/elif/while) and '{'
    pub(crate) fn parse_condition(&mut self) -> String {
        let mut condition = String::new();
        while self.cursor < self.tokens.len() && !matches!(self.tokens[self.cursor], Token::LBrace) {
            condition.push_str(&self.tokens[self.cursor].to_string());
            condition.push(' ');
            self.cursor += 1;
        }
        condition.trim().to_string()
    }
}