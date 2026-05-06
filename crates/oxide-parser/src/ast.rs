#[derive(Debug, PartialEq, Clone)]
pub enum Condition {
    Always,
    And,   
    Or,    
}

#[derive(Debug, PartialEq, Clone)]
pub struct Executable {
    pub statement: Statement,
    pub condition: Condition,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Command {
    pub program: String,
    pub args: Vec<String>,
    pub outfile: Option<String>, // Parser uses this for '>'
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

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    Command(Command), 
    Pipeline(Vec<Command>),
    If {
        condition: String,
        then_branch: Vec<Statement>,
        else_branch: Option<Vec<Statement>>,
    },
}