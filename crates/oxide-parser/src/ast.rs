#[derive(Debug, PartialEq, Clone)]
pub enum Condition {
    Always, // Normal execution
    And,    // Only run if the previous command SUCCEEDED
    Or,     // Only run if the previous command FAILED
}

#[derive(Debug, PartialEq, Clone)]
pub struct Executable {
    pub statement: Statement,
    pub condition: Condition,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Command {
    pub program: String,
    pub args: Vec<String>,
    pub outfile: Option<String>,
}

#[derive(Debug, PartialEq, Clone)]
pub enum Statement {
    SimpleCommand(Command),
    Pipeline(Vec<Command>), 
}