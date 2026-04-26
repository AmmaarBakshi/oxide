#[derive(Debug, PartialEq, Clone)]
pub enum Token {
    /// A standard command or argument (e.g., `ls`, `-al`, `main.rs`)
    Word(String),
    /// A string literal enclosed in quotes (e.g., `"hello world"`)
    StringLiteral(String),
    // Later we will add things like:
    // Pipe,          // |
    // RedirectOut,   // >
    // And,           // &&
    RedirectOut,
}