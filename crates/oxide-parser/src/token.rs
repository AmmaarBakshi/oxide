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
    RedirectAppend,    // >>
    RedirectIn,        // Add this
    RedirectErr,       // 2>
    RedirectErrAppend, // 2>>
    Background,        // Add this
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
            Token::RedirectAppend => write!(f, ">>"),
            Token::RedirectIn => write!(f, "<"), // Add this
            Token::RedirectErr => write!(f, "2>"),
            Token::RedirectErrAppend => write!(f, "2>>"),
            Token::Background => write!(f, "&"), // Add this
            Token::Newline => writeln!(f),
            Token::Semicolon => write!(f, ";"),
        }
    }
}
