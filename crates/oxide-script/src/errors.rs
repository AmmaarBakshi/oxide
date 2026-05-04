use thiserror::Error;

#[derive(Error, Debug)]
pub enum ScriptError {
    #[error("Variable '{0}' not defined")]
    UndefinedVariable(String),
    #[error("Syntax error at line {0}: {1}")]
    SyntaxError(usize, String),
    #[error("Runtime error: {0}")]
    RuntimeError(String),
}