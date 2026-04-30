#[derive(Debug, PartialEq, Clone)]
pub enum Token {
    Word(String),
    StringLiteral(String),
    Pipe, 
    And,
    Or,
    RedirectOut,    // >
    RedirectIn,     // <
    Background,     // &
}