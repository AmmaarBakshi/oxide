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
    If {
        condition: String,
        body: Vec<Statement>,
        else_if: Vec<(String, Vec<Statement>)>, // (condition, body)
        else_body: Option<Vec<Statement>>,
    },
    Command(String, Vec<String>),
    Pipeline(Vec<Statement>), // Add this variant
}