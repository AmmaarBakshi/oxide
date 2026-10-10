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
    ErrToOut,          // 2>&1
    OutToErr,          // >&2
    RedirectAll,       // &>
    RedirectAllAppend, // &>>
    /// `<<<`: the next word is fed to stdin.
    HereString,
    /// `<<EOF` with the body the lexer collected from the lines after it.
    HereDoc {
        body: String,
        expand: bool,
    },
    Background, // Add this
    Newline,
    Semicolon,
    /// `;;`, which ends an arm of a `case`.
    DoubleSemi,
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
            Token::ErrToOut => write!(f, "2>&1"),
            Token::OutToErr => write!(f, ">&2"),
            Token::RedirectAll => write!(f, "&>"),
            Token::RedirectAllAppend => write!(f, "&>>"),
            Token::HereDoc { .. } => write!(f, "<<"),
            Token::HereString => write!(f, "<<<"),
            Token::Background => write!(f, "&"), // Add this
            Token::Newline => writeln!(f),
            Token::Semicolon => write!(f, ";"),
            Token::DoubleSemi => write!(f, ";;"),
        }
    }
}
