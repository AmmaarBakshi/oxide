#[derive(Debug, PartialEq, Clone)]
pub enum Token {
    Word(String),
    StringLiteral(String),
    RedirectOut,
    Pipe, 
    And,
    Or,
}