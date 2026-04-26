use crate::shell::Shell;
use std::fs::File;
use std::process::{Command, Stdio};
use std::env;

use oxide_parser::lexer::Lexer;
use oxide_parser::parser::Parser;
use oxide_parser::ast::{Statement, Condition};

// --- NEW: Rustyline Imports ---
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;

impl Shell {
    pub fn run_repl(&mut self) -> anyhow::Result<()> {
        println!("⚗️ Oxide Shell Core v0.1.0");

        // --- NEW: Initialize the Terminal Editor ---
        let mut rl = DefaultEditor::new()?;
        
        // Try to load history from a file (it will silently fail if the file doesn't exist yet, which is fine!)
        let _ = rl.load_history("history.txt");

        while self.state.is_running {
            let cwd = env::current_dir()?;
            let prompt = format!("oxide {} > ", cwd.display());

            // --- NEW: Rustyline takes over reading input ---
            let readline = rl.readline(&prompt);
            
            let input = match readline {
                Ok(line) => {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    
                    // Save the command to our history so the UP arrow works!
                    rl.add_history_entry(trimmed)?;
                    
                    if trimmed == "exit" {
                        self.state.is_running = false;
                        break; // Break out of the loop completely
                    }
                    
                    // Return the trimmed string to be parsed
                    trimmed.to_string()
                },
                Err(ReadlineError::Interrupted) => {
                    // They pressed CTRL-C. Just give them a new prompt!
                    println!("^C");
                    continue;
                },
                Err(ReadlineError::Eof) => {
                    // They pressed CTRL-D. This means "End of File" / Exit.
                    println!("exit");
                    self.state.is_running = false;
                    break;
                },
                Err(err) => {
                    println!("Error: {:?}", err);
                    break;
                }
            };

            // ==========================================
            // --- NEW: THE PRE-PROCESSOR ---
            // ==========================================
            // Check if the very first word in the input matches an alias!
            let mut processed_input = input.clone();
            if let Some(first_word) = input.split_whitespace().next() {
                if let Some(replacement) = self.state.aliases.get(first_word) {
                    // Replace ONLY the first occurrence (the command itself)
                    processed_input = input.replacen(first_word, replacement, 1);
                }
            }
            
            // ==========================================
            // YOUR EXISTING PARSER & EXECUTION ENGINE 
            // ==========================================
            let mut lexer = Lexer::new(&processed_input);
            let tokens = lexer.tokenize();

            let mut parser = Parser::new(tokens);
            let executables = parser.parse(); // <-- 1. This is 'executables' now!

            for exec in executables { // <-- 2. Looping over 'exec' in 'executables'
                
                // --- NEW: CONTROL FLOW LOGIC ---
                match exec.condition {
                    Condition::And if self.state.last_exit_code != 0 => continue, // Skip if previous failed
                    Condition::Or if self.state.last_exit_code == 0 => continue,  // Skip if previous succeeded
                    _ => {} // Otherwise, proceed!
                }

                match exec.statement { // <-- 3. Matching on exec.statement
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
                        // --- CHECK BUILT-INS FIRST ---
                        if cmd.program == "alias" {
                            if cmd.args.is_empty() {
                                // If they just type 'alias', list them all
                                for (key, val) in &self.state.aliases {
                                    println!("alias {}='{}'", key, val);
                                }
                                self.state.last_exit_code = 0;
                            } else {
                                // Save the new alias (e.g., alias ls="dir")
                                for arg in &cmd.args {
                                    if let Some((key, value)) = arg.split_once('=') {
                                        // Strip quotes from the value if they added them
                                        let clean_value = value.trim_matches(|c| c == '"' || c == '\'');
                                        self.state.aliases.insert(key.to_string(), clean_value.to_string());
                                    } else {
                                        eprintln!("oxide: alias: invalid format. Use name=value");
                                    }
                                }
                                self.state.last_exit_code = 0;
                            }
                            continue;
                        } else if cmd.program == "export" {
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

        // --- NEW: Save history when shutting down ---
        let _ = rl.save_history("history.txt");
        Ok(())
    }
}