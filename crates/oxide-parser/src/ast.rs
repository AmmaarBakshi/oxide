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

// --- ADD THIS BLOCK ---
impl Command {
    pub fn new(program: String) -> Self {
        Self {
            program,
            args: Vec::new(),
            outfile: None,
        }
    }
}