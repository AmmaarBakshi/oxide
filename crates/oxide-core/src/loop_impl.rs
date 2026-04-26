use crate::shell::Shell;
use std::io::{self, Write};
use std::process::Command;
use std::env; 

use oxide_parser::lexer::Lexer;
use oxide_parser::parser::Parser;
use oxide_parser::ast::Statement;
use std::fs::File;            
use std::process::Stdio;      

impl Shell {
    pub fn run_repl(&mut self) -> anyhow::Result<()> {
        println!("⚗️ Oxide Shell Core v0.1.0");

        while self.state.is_running {
            // --- NEW: DYNAMIC PROMPT ---
            // Grab the current directory and display it
            let cwd = env::current_dir()?;
            print!("oxide {} > ", cwd.display());
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;

            let input = input.trim();
            if input == "exit" {
                self.state.is_running = false;
                continue;
            }
            if input.is_empty() {
                continue;
            }

            let mut lexer = Lexer::new(input);
            let tokens = lexer.tokenize();

            let mut parser = Parser::new(tokens);
            let statements = parser.parse();

            for stmt in statements {
                match stmt {
                    // ==========================================
                    // ARM 1: SINGLE COMMANDS (No Pipes)
                    // ==========================================
                    Statement::SimpleCommand(cmd) => {
                        let mut expanded_args = Vec::new();
                        for arg in &cmd.args {
                            if arg.starts_with('$') {
                                let var_name = &arg[1..];
                                let val = std::env::var(var_name).unwrap_or_default();
                                expanded_args.push(val);
                            } else {
                                expanded_args.push(arg.clone());
                            }
                        }

                        if cmd.program == "export" {
                            self.state.last_exit_code = oxide_builtins::export::execute(&expanded_args);
                            continue;
                        } else if cmd.program == "echo" {
                            self.state.last_exit_code = oxide_builtins::echo::execute(&expanded_args);
                            continue;
                        } else if cmd.program == "cd" { 
                            self.state.last_exit_code = oxide_builtins::cd::execute(&expanded_args);
                            continue;
                        }

                        let mut process = Command::new(&cmd.program);
                        process.args(&expanded_args);

                        if let Some(file_name) = &cmd.outfile {
                            match File::create(file_name) {
                                Ok(file) => {
                                    process.stdout(Stdio::from(file));
                                }
                                Err(e) => {
                                    eprintln!("oxide: failed to open file {}: {}", file_name, e);
                                    self.state.last_exit_code = 1;
                                    continue;
                                }
                            }
                        }

                        match process.spawn() {
                            Ok(mut child) => { 
                                let status = child.wait()?;
                                self.state.last_exit_code = status.code().unwrap_or(1);
                            },
                            Err(_) => {
                                eprintln!("oxide: command not found: {}", cmd.program);
                                self.state.last_exit_code = 127;
                            }
                        }
                    }

                    // ==========================================
                    // ARM 2: PIPELINES (A | B | C)
                    // ==========================================
                    Statement::Pipeline(commands) => {
                        let mut previous_stdout = None;
                        let len = commands.len();

                        for (i, cmd) in commands.iter().enumerate() {
                            let mut expanded_args = Vec::new();
                            for arg in &cmd.args {
                                if arg.starts_with('$') {
                                    let var_name = &arg[1..];
                                    let val = std::env::var(var_name).unwrap_or_default();
                                    expanded_args.push(val);
                                } else {
                                    expanded_args.push(arg.clone());
                                }
                            }

                            if cmd.program == "export" {
                                self.state.last_exit_code = oxide_builtins::export::execute(&expanded_args);
                                continue;
                            } else if cmd.program == "cd" {
                                self.state.last_exit_code = oxide_builtins::cd::execute(&expanded_args);
                                continue;
                            } else if cmd.program == "echo" {
                                self.state.last_exit_code = oxide_builtins::echo::execute(&expanded_args);
                                continue;
                            }

                            let mut process = Command::new(&cmd.program);
                            process.args(&expanded_args);

                            if let Some(stdout) = previous_stdout.take() {
                                process.stdin(Stdio::from(stdout));
                            }

                            if i < len - 1 {
                                process.stdout(Stdio::piped());
                            } else if let Some(file_name) = &cmd.outfile {
                                if let Ok(file) = File::create(file_name) {
                                    process.stdout(Stdio::from(file));
                                }
                            }

                            match process.spawn() {
                                Ok(mut child) => {
                                    if i < len - 1 {
                                        previous_stdout = child.stdout.take();
                                    } else {
                                        let status = child.wait()?;
                                        self.state.last_exit_code = status.code().unwrap_or(1);
                                    }
                                }
                                Err(_) => {
                                    eprintln!("oxide: command not found: {}", cmd.program);
                                    self.state.last_exit_code = 127;
                                    break; 
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }
}