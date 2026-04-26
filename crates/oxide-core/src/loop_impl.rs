use crate::shell::Shell;
use std::fs::File;
use std::process::{Command, Stdio};
use std::env;
use std::io::{BufRead, BufReader}; // <-- NEW: For reading files

use oxide_parser::lexer::Lexer;
use oxide_parser::parser::Parser;
use oxide_parser::ast::{Statement, Condition};

use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;

impl Shell {
    // ==========================================
    // MODE 1: INTERACTIVE KEYBOARD (REPL)
    // ==========================================
    pub fn run_repl(&mut self) -> anyhow::Result<()> {
        println!(" Oxide Shell Core v0.1.0");
        let mut rl = DefaultEditor::new()?;
        let _ = rl.load_history("history.txt");

        while self.state.is_running {
            let cwd = env::current_dir()?;
            let cwd_str = cwd.display().to_string();
            let raw_prompt = env::var("PROMPT").unwrap_or_else(|_| "oxide $CWD > ".to_string());
            let prompt = raw_prompt.replace("$CWD", &cwd_str);

            match rl.readline(&prompt) {
                Ok(line) => {
                    let trimmed = line.trim();
                    if trimmed.is_empty() { continue; }
                    rl.add_history_entry(trimmed)?;
                    if trimmed == "exit" {
                        self.state.is_running = false;
                        break;
                    }
                    self.execute_line(trimmed); // <-- Calls the Engine!
                },
                Err(ReadlineError::Interrupted) => { println!("^C"); continue; },
                Err(ReadlineError::Eof) => { self.state.is_running = false; break; },
                Err(err) => { println!("Error: {:?}", err); break; }
            }
        }
        let _ = rl.save_history("history.txt");
        Ok(())
    }

    // ==========================================
    // MODE 2: FILE AUTOMATION (SCRIPT)
    // ==========================================
    pub fn run_script(&mut self, file_path: &str) -> anyhow::Result<()> {
        let file = File::open(file_path)?;
        let reader = BufReader::new(file);

        for line in reader.lines() {
            let line = line?;
            let trimmed = line.trim();
            
            // Ignore empty lines and bash-style # comments
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if trimmed == "exit" {
                break;
            }

            self.execute_line(trimmed); // <-- Calls the exact same Engine!
        }
        Ok(())
    }

    // ==========================================
    // THE SHARED EXECUTION ENGINE
    // ==========================================
    pub fn execute_line(&mut self, input: &str) {
        let mut processed_input = input.to_string();
        if let Some(first_word) = input.split_whitespace().next() {
            if let Some(replacement) = self.state.aliases.get(first_word) {
                processed_input = input.replacen(first_word, replacement, 1);
            }
        }

        let mut lexer = Lexer::new(&processed_input);
        let tokens = lexer.tokenize();
        let mut parser = Parser::new(tokens);
        let executables = parser.parse();

        for exec in executables {
            match exec.condition {
                Condition::And if self.state.last_exit_code != 0 => continue,
                Condition::Or if self.state.last_exit_code == 0 => continue,
                _ => {}
            }

            match exec.statement {
                Statement::SimpleCommand(cmd) => {
                    let mut expanded_args = Vec::new();
                    for arg in &cmd.args {
                        if arg.starts_with('$') {
                            let val = std::env::var(&arg[1..]).unwrap_or_default();
                            expanded_args.push(val);
                        } else {
                            expanded_args.push(arg.clone());
                        }
                    }

                    if cmd.program == "alias" {
                        self.state.last_exit_code = oxide_builtins::alias::execute(&cmd.args, &mut self.state.aliases);
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
                    } else if cmd.program == "pwd" { 
                        self.state.last_exit_code = oxide_builtins::pwd::execute(&expanded_args);
                        continue;
                    }

                    let mut process = Command::new(&cmd.program);
                    process.args(&expanded_args);

                    if let Some(file_name) = &cmd.outfile {
                        if let Ok(file) = File::create(file_name) { process.stdout(Stdio::from(file)); }
                    }

                    match process.spawn() {
                        Ok(mut child) => { 
                            let status = child.wait().expect("failed to wait");
                            self.state.last_exit_code = status.code().unwrap_or(1);
                        },
                        Err(_) => {
                            eprintln!("oxide: command not found: {}", cmd.program);
                            self.state.last_exit_code = 127;
                        }
                    }
                }
                Statement::Pipeline(commands) => {
                    let mut previous_stdout = None;
                    let len = commands.len();

                    for (i, cmd) in commands.iter().enumerate() {
                        let mut expanded_args = Vec::new();
                        for arg in &cmd.args {
                            if arg.starts_with('$') {
                                let val = std::env::var(&arg[1..]).unwrap_or_default();
                                expanded_args.push(val);
                            } else {
                                expanded_args.push(arg.clone());
                            }
                        }

                        if cmd.program == "pwd" { 
                            self.state.last_exit_code = oxide_builtins::pwd::execute(&expanded_args);
                            continue;
                        }

                        else if cmd.program == "alias" {
                            self.state.last_exit_code = oxide_builtins::alias::execute(&cmd.args, &mut self.state.aliases);
                            continue;
                        }

                        else if cmd.program == "export" { 
                            self.state.last_exit_code = oxide_builtins::export::execute(&expanded_args); 
                            continue; 
                        }

                        else if cmd.program == "cd" { 
                            self.state.last_exit_code = oxide_builtins::cd::execute(&expanded_args); 
                            continue; 
                        }

                        else if cmd.program == "echo" { 
                            self.state.last_exit_code = oxide_builtins::echo::execute(&expanded_args); 
                            continue; 
                        }

                        let mut process = Command::new(&cmd.program);
                        process.args(&expanded_args);

                        if let Some(stdout) = previous_stdout.take() { process.stdin(Stdio::from(stdout)); }

                        if i < len - 1 { process.stdout(Stdio::piped()); } 
                        else if let Some(file_name) = &cmd.outfile {
                            if let Ok(file) = File::create(file_name) { process.stdout(Stdio::from(file)); }
                        }

                        match process.spawn() {
                            Ok(mut child) => {
                                if i < len - 1 { previous_stdout = child.stdout.take(); } 
                                else {
                                    let status = child.wait().expect("failed to wait");
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
}