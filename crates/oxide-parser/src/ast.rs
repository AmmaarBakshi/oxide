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