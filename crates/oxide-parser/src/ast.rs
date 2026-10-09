/// A `NAME=value` word written before a command, or on its own.
#[derive(Clone, Debug, PartialEq)]
pub struct Assignment {
    pub name: String,
    /// The value exactly as written; it is expanded when the command runs.
    pub value: String,
}

impl Assignment {
    /// Reads `word` as an assignment: a valid, unquoted variable name
    /// followed by `=`. Anything else (`=x`, `1X=2`, `"X"=1`) is not one.
    pub fn parse(word: &str) -> Option<Self> {
        let (name, value) = word.split_once('=')?;
        let mut chars = name.chars();
        let valid = matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
            && chars.all(|c| c.is_ascii_alphanumeric() || c == '_');
        valid.then(|| Self {
            name: name.to_string(),
            value: value.to_string(),
        })
    }
}

#[derive(Clone, Debug)]
pub struct Command {
    /// The program to run. Empty for a line of bare assignments (`X=1`).
    pub program: String,
    pub args: Vec<String>,
    /// `NAME=value` words that came before the program.
    pub assignments: Vec<Assignment>,
    pub outfile: Option<String>,
    pub infile: Option<String>,
    /// When true, `outfile` is opened in append mode (`>>`) rather than truncating (`>`).
    pub append: bool,
    /// Where stderr goes (`2>` / `2>>`).
    pub errfile: Option<String>,
    /// When true, `errfile` is appended to (`2>>`).
    pub err_append: bool,
}

impl Command {
    pub fn new(program: String) -> Self {
        Self {
            program,
            args: Vec::new(),
            assignments: Vec::new(),
            outfile: None,
            infile: None,
            append: false,
            errfile: None,
            err_append: false,
        }
    }
}

#[derive(Clone, Debug)]
pub enum Condition {
    Always,
    And,
    Or,
}

pub struct Executable {
    pub statement: Statement,
    pub condition: Condition,
}

pub enum Statement {
    If {
        condition: String,
        body: Vec<Statement>,
        else_if: Vec<(String, Vec<Statement>)>,
        else_body: Option<Vec<Statement>>,
    },
    While {
        condition: String,
        body: Vec<Statement>,
    },
    For {
        variable: String,
        values: Vec<String>,
        body: Vec<Statement>,
    },
    Command(Command),
    Pipeline(Vec<Command>),
}
