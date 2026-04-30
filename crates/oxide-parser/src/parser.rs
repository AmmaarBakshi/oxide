use crate::token::Token;
use crate::ast::{Command, Statement, Condition, Executable};

pub struct Parser {
    tokens: Vec<Token>,
    cursor: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, cursor: 0 }
    }

    /// Turns the flat list of tokens into a structured execution tree
    pub fn parse(&mut self) -> Vec<Executable> {
        let mut executables = Vec::new();
        let mut current_condition = Condition::Always;

        while self.cursor < self.tokens.len() {
            // 1. Parse a full statement (either a simple command or a pipeline)
            if let Some(statement) = self.parse_statement() {
                executables.push(Executable {
                    statement,
                    condition: current_condition.clone(),
                });
            }

            // 2. Check what comes after the statement (like && or ||)
            if self.cursor < self.tokens.len() {
                match &self.tokens[self.cursor] {
                    Token::And => {
                        current_condition = Condition::And;
                        self.cursor += 1;
                    }
                    Token::Or => {
                        current_condition = Condition::Or;
                        self.cursor += 1;
                    }
                    _ => {
                        // Reset condition for the next line/command
                        current_condition = Condition::Always;
                    }
                }
            }
        }

        executables
    }

    /// Helper to parse a group of tokens until it hits a boundary (&&, ||, or end)
    fn parse_statement(&mut self) -> Option<Statement> {
        let mut pipeline = Vec::new();
        let mut current_cmd: Option<Command> = None;

        while self.cursor < self.tokens.len() {
            let token = &self.tokens[self.cursor];

            match token {
                // Handle Normal Words and our new Quoted Strings!
                Token::Word(w) | Token::StringLiteral(w) => {
                    if let Some(cmd) = &mut current_cmd {
                        cmd.args.push(w.clone()); // It's an argument
                    } else {
                        current_cmd = Some(Command::new(w.clone())); // It's a new program!
                    }
                    self.cursor += 1;
                }
                
                // Handle Pipes (Save the current command and prepare for the next one)
                Token::Pipe => {
                    if let Some(cmd) = current_cmd.take() {
                        pipeline.push(cmd);
                    }
                    self.cursor += 1;
                }
                
                // Handle File Redirection Output (>)
                Token::RedirectOut => {
                    self.cursor += 1; // Consume the '>'
                    
                    // The very next token MUST be the file name
                    if self.cursor < self.tokens.len() {
                        if let Token::Word(outfile) | Token::StringLiteral(outfile) = &self.tokens[self.cursor] {
                            if let Some(cmd) = &mut current_cmd {
                                cmd.outfile = Some(outfile.clone());
                            }
                            self.cursor += 1;
                        }
                    }
                }
                
                // Handle File Redirection Input (<)
                Token::RedirectIn => {
                    self.cursor += 1; // We will skip this for now!
                }
                
                // Handle Background Jobs (&)
                Token::Background => {
                    if let Some(cmd) = &mut current_cmd {
                        cmd.args.push("&".to_string()); // Executor expects this
                    }
                    self.cursor += 1;
                    break; // Background symbol usually ends the statement
                }
                
                // Stop parsing the statement if we hit conditional logic
                Token::And | Token::Or => {
                    break; 
                }
            }
        }

        // Push the final command we were building
        if let Some(cmd) = current_cmd {
            pipeline.push(cmd);
        }

        // Return the correct AST node
        if pipeline.is_empty() {
            None
        } else if pipeline.len() == 1 {
            Some(Statement::SimpleCommand(pipeline.pop().unwrap()))
        } else {
            Some(Statement::Pipeline(pipeline))
        }
    }
}