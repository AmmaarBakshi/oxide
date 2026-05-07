use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Word(String),
    StringLiteral(String),
    If,
    Elif,
    Else,
    LBrace,
    RBrace,
    And,
    Or,
    Pipe,
    RedirectOut,
    RedirectIn,  // Add this
    Background,  // Add this
    Newline,
    Semicolon,
    While,
    For,
    In,
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Token::Word(s) | Token::StringLiteral(s) => write!(f, "{}", s),
            Token::If => write!(f, "if"),
            Token::Elif => write!(f, "elif"),
            Token::Else => write!(f, "else"),
            Token::While => write!(f, "while"),
            Token::For => write!(f, "for"),
            Token::In => write!(f, "in"),
            Token::LBrace => write!(f, "{{"),
            Token::RBrace => write!(f, "}}"),
            Token::And => write!(f, "&&"),
            Token::Or => write!(f, "||"),
            Token::Pipe => write!(f, "|"),
            Token::RedirectOut => write!(f, ">"),
            Token::RedirectIn => write!(f, "<"),     // Add this
            Token::Background => write!(f, "&"),     // Add this
            Token::Newline => write!(f, "\n"),
            Token::Semicolon => write!(f, ";"),
        }
    }
}