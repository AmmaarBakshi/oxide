#[derive(Debug, PartialEq, Clone)]
pub enum Token {
    Word(String),
    StringLiteral(String),
    RedirectOut,
    Pipe, // <-- NEW: Represents the '|' symbol
}