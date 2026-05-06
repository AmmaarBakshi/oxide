#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Word(String),
    StringLiteral(String),
    Pipe,          // |
    RedirectOut,   // >
    RedirectIn,    // <
    Background,    // &
    And,           // &&
    Or,            // ||
    LBrace,        // {  <-- Add this
    RBrace,        // }  <-- Add this
    // ... any other tokens you have
}