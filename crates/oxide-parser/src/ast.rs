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

/// Text fed to a command's stdin from the command line itself.
#[derive(Clone, Debug, PartialEq)]
pub enum HereInput {
    /// `<<EOF` ... `EOF`. `expand` is false when the delimiter was quoted
    /// (`<<'EOF'`), which keeps `$` in the body literal.
    Document { body: String, expand: bool },
    /// `<<< word`: the word, expanded, plus a newline.
    String(String),
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
    /// `2>&1` (or `&>`): stderr goes wherever stdout goes.
    pub err_to_out: bool,
    /// `>&2`: stdout goes wherever stderr goes.
    pub out_to_err: bool,
    /// A here-document for stdin.
    pub here: Option<HereInput>,
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
            err_to_out: false,
            out_to_err: false,
            here: None,
        }
    }
}

#[derive(Clone, Debug)]
pub enum Condition {
    Always,
    And,
    Or,
}

/// One statement in a list, and how it hangs off the one before: always
/// run, or only after a success (`&&`) or a failure (`||`).
#[derive(Clone, Debug)]
pub struct Executable {
    pub statement: Statement,
    pub condition: Condition,
}

/// Statements joined by newlines, `;`, `&&` and `||`: a block's body, an
/// `if` or `while` condition, or a whole line.
pub type List = Vec<Executable>;

#[derive(Clone, Debug)]
pub enum Statement {
    /// `if LIST { ... } elif LIST { ... } else { ... }`. Each condition is
    /// a list of commands; the branch taken is the first whose condition
    /// exits with status 0.
    If {
        condition: List,
        body: List,
        else_if: Vec<(List, List)>,
        else_body: Option<List>,
    },
    /// `while LIST { ... }`: runs the body for as long as the condition
    /// exits with status 0.
    While {
        condition: List,
        body: List,
    },
    For {
        variable: String,
        values: Vec<String>,
        body: List,
    },
    Command(Command),
    Pipeline(Vec<Command>),
}
