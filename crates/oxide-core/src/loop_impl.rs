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
                    Statement::SimpleCommand(cmd) => {
                        
                        // --- CHECK FOR BUILT-INS ---
                        if cmd.program == "echo" {
                            self.state.last_exit_code = oxide_builtins::echo::execute(&cmd.args);
                            continue;
                        } else if cmd.program == "cd" { // <-- NEW CD INTERCEPT
                            self.state.last_exit_code = oxide_builtins::cd::execute(&cmd.args);
                            continue;
                        }

                        // --- SPAWN SYSTEM PROCESS ---
                        let mut process = Command::new(&cmd.program);
                        process.args(&cmd.args);

                        // --- NEW: HANDLE REDIRECTION ---
                        if let Some(file_name) = &cmd.outfile {
                            // Try to create/open the file
                            match File::create(file_name) {
                                Ok(file) => {
                                    // Tell the process to send standard output to this file
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

                    // --- NEW: EXECUTE A PIPELINE ---
                    Statement::Pipeline(commands) => {
                        let mut previous_stdout = None;
                        let len = commands.len();

                        for (i, cmd) in commands.iter().enumerate() {
                            let mut process = Command::new(&cmd.program);
                            process.args(&cmd.args);

                            // --- PHASE 1: PLUMBING (BEFORE SPAWN) ---
                            
                            // 1. If there was a previous command, pipe its output into this command's input
                            if let Some(stdout) = previous_stdout.take() {
                                process.stdin(Stdio::from(stdout));
                            }

                            // 2. If this is NOT the last command, capture its output for the next one
                            if i < len - 1 {
                                process.stdout(Stdio::piped());
                            } else if let Some(file_name) = &cmd.outfile {
                                // 3. The last command might have redirection!
                                if let Ok(file) = File::create(file_name) {
                                    process.stdout(Stdio::from(file));
                                }
                            }

                            // --- PHASE 2: EXECUTION (AFTER SPAWN) ---
                            match process.spawn() {
                                Ok(mut child) => {
                                    if i < len - 1 {
                                        // Save the stdout so the next iteration can use it
                                        previous_stdout = child.stdout.take();
                                    } else {
                                        // Only wait for the VERY LAST command in the chain to finish
                                        let status = child.wait()?;
                                        self.state.last_exit_code = status.code().unwrap_or(1);
                                    }
                                }
                                Err(_) => {
                                    eprintln!("oxide: command not found: {}", cmd.program);
                                    self.state.last_exit_code = 127;
                                    break; // Stop the pipeline
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