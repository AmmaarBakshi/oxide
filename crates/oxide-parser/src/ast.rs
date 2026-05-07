pub struct Command {
    pub program: String,
    pub args: Vec<String>,
    pub outfile: Option<String>,
}

impl Command {
    pub fn new(program: String) -> Self {
        Self {
            program,
            args: Vec::new(),
            outfile: None,
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