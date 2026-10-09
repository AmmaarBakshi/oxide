use crate::ast::List;
use crate::parser::Parser;
use crate::token::Token;

impl Parser {
    /// Parses the condition of an `if`, `elif` or `while`: a list of
    /// commands running up to the `{` that opens the body.
    pub(crate) fn parse_condition(&mut self) -> List {
        self.parse_list(|token| *token == Token::LBrace)
    }
}
