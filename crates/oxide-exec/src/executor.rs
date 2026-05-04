use std::collections::HashMap;
use oxide_compat::CompatMode;
use oxide_parser::lexer::Lexer;
use oxide_parser::parser::Parser;
use oxide_parser::ast::{Statement, Condition};

pub struct Executor;

impl Executor {
    pub fn new() -> Self {
        Self
    }

    pub fn execute_line(
        &mut self,
        input: &str,
        mode: &mut CompatMode,
        aliases: &mut HashMap<String, String>,
        last_exit_code: &mut i32,
        job_manager: &mut crate::jobs::JobManager,
        history: &[String], // Ensure this is passed from main.rs
    ) {
        // --- 1. Initialize Security Gatekeeper ---
        let security = oxide_security::permissions::PermissionManager::new();

        // Translate input based on compatibility mode
        let compat_input = match *mode {
            CompatMode::Bash => oxide_compat::bash_mode::translate(input),
            CompatMode::Posix => oxide_compat::posix_mode::translate(input),
            CompatMode::Oxide => input.to_string(),
        };

        // Handle Aliases
        let mut processed_input = compat_input.clone();
        if let Some(first_word) = compat_input.split_whitespace().next() {
            if let Some(replacement) = aliases.get(first_word) {
                processed_input = compat_input.replacen(first_word, replacement, 1);
            }
        }

        // --- SUBSHELL INTERCEPTOR ---
        let trimmed = processed_input.trim();
        if trimmed.starts_with('(') && trimmed.ends_with(')') {
            let inner_cmd = &trimmed[1..trimmed.len() - 1];
            *last_exit_code = crate::subshell::execute(inner_cmd, mode, aliases, job_manager, history);
            return;
        }

        // Tokenize and Parse
        let mut lexer = Lexer::new(&processed_input);
        let tokens = lexer.tokenize();
        let mut parser = Parser::new(tokens);
        let executables = parser.parse();

        for exec in executables {
            // --- LOGIC GATES (&& and ||) ---
            match exec.condition {
                Condition::And => if *last_exit_code != 0 { continue; },
                Condition::Or => if *last_exit_code == 0 { continue; },
                Condition::Always => {}
            }

            match exec.statement {
                Statement::SimpleCommand(cmd) => {
                    // Pre-process arguments (Expansion & Globs)
                    let mut expanded_args: Vec<String> = Vec::new();
                    for arg in &cmd.args {
                        let text_expanded = oxide_parser::expand::expand_text(arg);
                        expanded_args.extend(oxide_parser::glob::expand_glob(&text_expanded));
                    }

                    // --- 2. SECURITY CHECK ---
                    if let Err(e) = security.is_allowed(&cmd.program, &expanded_args) {
                        oxide_security::audit::log_command(&cmd.program, &expanded_args, false);
                        eprintln!("{}", e);
                        *last_exit_code = 1;
                        continue; 
                    }
                    oxide_security::audit::log_command(&cmd.program, &expanded_args, true);

                    // --- BUILT-IN ROUTING ---
                    match cmd.program.as_str() {
                        "mode" => {
                            if expanded_args.is_empty() {
                                println!("oxide: current mode is {:?}", *mode);
                            } else {
                                match expanded_args[0].as_str() {
                                    "bash" => *mode = CompatMode::Bash,
                                    "posix" => *mode = CompatMode::Posix,
                                    "oxide" => *mode = CompatMode::Oxide,
                                    _ => eprintln!("oxide: unknown mode"),
                                }
                            }
                            *last_exit_code = 0;
                        }
                        "alias" => *last_exit_code = oxide_builtins::alias::execute(&cmd.args, aliases),
                        "cd" => *last_exit_code = oxide_builtins::cd::execute(&expanded_args),
                        "pwd" => *last_exit_code = oxide_builtins::pwd::execute(&expanded_args),
                        "ls" | "dir" => *last_exit_code = oxide_builtins::ls::execute(&expanded_args),
                        "echo" => {
                            let output = expanded_args.join(" ");
                            if let Some(filename) = &cmd.outfile {
                                if let Ok(mut file) = std::fs::File::create(filename) {
                                    use std::io::Write;
                                    let _ = writeln!(file, "{}", output);
                                }
                            } else {
                                println!("{}", output);
                            }
                            *last_exit_code = 0;
                        }
                        "touch" => *last_exit_code = oxide_builtins::touch::execute(&expanded_args),
                        "cat" => *last_exit_code = oxide_builtins::cat::execute(&expanded_args),
                        "env" => *last_exit_code = oxide_builtins::env::execute(),
                        "history" => *last_exit_code = oxide_builtins::history::execute(history),
                        "grep" => *last_exit_code = oxide_builtins::grep::execute(&expanded_args),
                        "jobs" => { job_manager.print_jobs(); *last_exit_code = 0; },
                        "clear" => *last_exit_code = oxide_builtins::clear::execute(&expanded_args),
                        "jail" => {
                            if expanded_args.is_empty() {
                                eprintln!("jail: usage: jail <command> [args]");
                                *last_exit_code = 1;
                            } else {
                                // Initialize a sandbox in a temporary folder
                                let sandbox = oxide_security::sandbox::Sandbox::new("./oxide_jail");
                                
                                let sub_program = &expanded_args[0];
                                let sub_args = &expanded_args[1..].to_vec();
                                
                                match sandbox.run(sub_program, sub_args) {
                                    Ok(code) => *last_exit_code = code,
                                    Err(e) => {
                                        eprintln!("{}", e);
                                        *last_exit_code = 1;
                                    }
                                }
                            }
                            continue;
                        }
                        
                        // --- OS FALLBACK ---
                        _ => {
                            let is_background = expanded_args.last().map(|s| s.as_str()) == Some("&");
                            let mut args = expanded_args.clone();
                            if is_background { args.pop(); }

                            if is_background {
                                match crate::process::spawn_background(&cmd.program, &args, &cmd.outfile) {
                                    Ok(child) => {
                                        job_manager.add(cmd.program.clone(), child);
                                        *last_exit_code = 0;
                                    }
                                    Err(e) => { eprintln!("{}", e); *last_exit_code = 127; }
                                }
                            } else {
                                *last_exit_code = crate::process::spawn_single(&cmd.program, &args, &cmd.outfile);
                            }
                        }
                    }
                }
                Statement::Pipeline(_commands) => {
                    // For now, call your existing pipeline logic here
                    // Ensure it also respects the security manager!
                }
            }
        }
    }
}