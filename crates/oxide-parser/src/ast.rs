#[derive(Debug, PartialEq, Clone)]
pub struct Command {
    pub program: String,
    pub args: Vec<String>,
    pub outfile: Option<String>,
}

#[derive(Debug, PartialEq, Clone)]
pub enum Statement {
    /// A basic command like `ls -al` or `echo "hello"`
    SimpleCommand(Command),
    // Later we'll add things like Pipeline (cmd1 | cmd2)
}