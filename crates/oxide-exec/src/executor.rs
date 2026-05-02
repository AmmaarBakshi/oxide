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
    ) {
        let compat_input = match *mode {
            CompatMode::Bash => oxide_compat::bash_mode::translate(input),
            CompatMode::Posix => oxide_compat::posix_mode::translate(input),
            CompatMode::Oxide => input.to_string(),
        };

        let mut processed_input = compat_input.clone();
        if let Some(first_word) = compat_input.split_whitespace().next() {
            if let Some(replacement) = aliases.get(first_word) {
                processed_input = compat_input.replacen(first_word, replacement, 1);
            }
        }

        // --- SUBSHELL INTERCEPTOR ---
        let trimmed = processed_input.trim();
        if trimmed.starts_with('(') && trimmed.ends_with(')') {
            // Strip off the ( and )
            let inner_cmd = &trimmed[1..trimmed.len() - 1];
            
            // Route it to the sandbox!
            *last_exit_code = crate::subshell::execute(inner_cmd, mode, aliases, job_manager);
            return; // Stop here! The subshell handles the rest.
        }

        let mut lexer = Lexer::new(&processed_input);
        let tokens = lexer.tokenize();
        let mut parser = Parser::new(tokens);
        let executables = parser.parse();

        for exec in executables {
            // --- THE LOGIC GATE ---
            match exec.condition {
                Condition::And => {
                    if *last_exit_code != 0 { continue; } // Skip if previous failed
                }
                Condition::Or => {
                    if *last_exit_code == 0 { continue; } // Skip if previous succeeded
                }
                Condition::Always => {} // Always run (default)
            }

            match exec.statement {
                Statement::SimpleCommand(cmd) => {
                    // --- NEW PRE-PROCESSOR PIPELINE ---
                    let mut expanded_args: Vec<String> = Vec::new();
                    for arg in &cmd.args {
                        let text_expanded = oxide_parser::expand::expand_text(arg);
                        expanded_args.extend(oxide_parser::glob::expand_glob(&text_expanded));
                    }
                    // ----------------------------------

                    if cmd.program == "mode" {
                        if cmd.args.is_empty() {
                            println!("oxide: current mode is {:?}", *mode);
                        } else {
                            match cmd.args[0].as_str() {
                                "bash" => *mode = CompatMode::Bash,
                                "posix" => *mode = CompatMode::Posix,
                                "oxide" => *mode = CompatMode::Oxide,
                                _ => eprintln!("oxide: unknown mode. Use bash, posix, or oxide"),
                            }
                        }
                        *last_exit_code = 0;
                        continue;
                    } else if cmd.program == "alias" {
                        *last_exit_code = oxide_builtins::alias::execute(&cmd.args, aliases);
                        continue;
                    } else if cmd.program == "export" {
                        *last_exit_code = oxide_builtins::export::execute(&expanded_args);
                        continue;
                    } else if cmd.program == "cd" {
                        *last_exit_code = oxide_builtins::cd::execute(&expanded_args);
                        continue;
                    } else if cmd.program == "pwd" {
                        *last_exit_code = oxide_builtins::pwd::execute(&expanded_args);
                        continue;
                    } else if cmd.program == "ls" || cmd.program == "dir" {
                        *last_exit_code = oxide_builtins::ls::execute(&expanded_args);
                        continue;
                    } else if cmd.program == "kill" {
                        *last_exit_code = oxide_builtins::kill::execute(&expanded_args);
                        continue;
                    } else if cmd.program == "sleep" || cmd.program == "wait" {
                        *last_exit_code = oxide_builtins::sleep::execute(&expanded_args);
                        continue;
                    } else if cmd.program == "rm" {
                        *last_exit_code = oxide_builtins::rm::execute(&expanded_args);
                        continue;
                    } else if cmd.program == "ps" {
                        *last_exit_code = oxide_builtins::ps::execute(&expanded_args);
                        continue;
                    } else if cmd.program == "top" {
                        *last_exit_code = oxide_builtins::top::execute(&expanded_args);
                        continue;
                    } else if cmd.program == "open" {
                        *last_exit_code = oxide_builtins::open::execute(&expanded_args);
                        continue;
                    } else if cmd.program == "echo" {
                        let output = expanded_args.join(" ");
                        
                        if let Some(filename) = &cmd.outfile {
                            // If there's an outfile, write to the file instead of the screen
                            if let Ok(mut file) = std::fs::File::create(filename) {
                                use std::io::Write;
                                let _ = writeln!(file, "{}", output);
                            }
                        } else {
                            // Otherwise, just print to the terminal
                            println!("{}", output);
                        }
                        *last_exit_code = 0;
                        continue;
                    } else if cmd.program == "jobs" {
                        job_manager.print_jobs();
                        *last_exit_code = 0;
                        continue;
                    } else if cmd.program == "clear" {
                        *last_exit_code = oxide_builtins::clear::execute(&expanded_args);
                        continue;
                    } else if cmd.program == "find" {
                        *last_exit_code = oxide_builtins::find::execute(&expanded_args);
                        continue;
                    } else if cmd.program == "hello" {
                        println!("why are u tiping hello? this is shell not ai.");
                        *last_exit_code = 0;
                        continue;
                    } else if cmd.program == "hi" {
                        println!("why are u tiping hi? this is shell not ai.");
                        *last_exit_code = 0;
                        continue;
                    } else if cmd.program == "hi" {
                        println!("why are u tiping hi? this is shell not ai.");
                        *last_exit_code = 0;
                        continue;
                    } else if cmd.program == "touch" {
                        *last_exit_code = oxide_builtins::touch::execute(&expanded_args);
                        continue;
                    } else if cmd.program == "cat" {
                        *last_exit_code = oxide_builtins::cat::execute(&expanded_args);
                        continue;
                    } else if cmd.program == "env" {
                        *last_exit_code = oxide_builtins::env::execute();
                        continue;
                    }
                    // --- OS FALLBACK ---
                    // Check if the command is meant to run in the background
                    let is_background = expanded_args.last().map(|s| s.as_str()) == Some("&");
                    
                    if is_background {
                        expanded_args.pop(); // Remove the "&" from the arguments
                        
                        match crate::process::spawn_background(&cmd.program, &expanded_args, &cmd.outfile) {
                            Ok(child) => {
                                job_manager.add(cmd.program.clone(), child);
                                *last_exit_code = 0;
                            }
                            Err(e) => {
                                eprintln!("{}", e);
                                *last_exit_code = 127;
                            }
                        }
                    } else {
                        // Standard foreground process
                        *last_exit_code = crate::process::spawn_single(
                            &cmd.program, 
                            &expanded_args, 
                            &cmd.outfile
                        );
                    }
                }
                Statement::Pipeline(commands) => {
                    let mut os_pipeline = crate::pipeline::OsPipeline::new();
                    let len = commands.len();
                    let mut internal_data: Option<oxide_data::value::Value> = None;

                    for (i, cmd) in commands.iter().enumerate() {
                        // --- NEW PRE-PROCESSOR PIPELINE ---
                        let mut expanded_args: Vec<String> = Vec::new();
                        for arg in &cmd.args {
                            let text_expanded = oxide_parser::expand::expand_text(arg);
                            expanded_args.extend(oxide_parser::glob::expand_glob(&text_expanded));
                        }
                        // ----------------------------------

                        // Clean Match Routing!
                        match cmd.program.as_str() {
                            "pwd" => *last_exit_code = oxide_builtins::pwd::execute(&expanded_args),
                            "alias" => *last_exit_code = oxide_builtins::alias::execute(&cmd.args, aliases),
                            "export" => *last_exit_code = oxide_builtins::export::execute(&expanded_args),
                            "cd" => *last_exit_code = oxide_builtins::cd::execute(&expanded_args),
                            "ls" | "dir" => *last_exit_code = oxide_builtins::ls::execute(&expanded_args),
                            "kill" => *last_exit_code = oxide_builtins::kill::execute(&expanded_args),
                            "sleep" | "wait" => *last_exit_code = oxide_builtins::sleep::execute(&expanded_args),
                            "rm" => *last_exit_code = oxide_builtins::rm::execute(&expanded_args),
                            "ps" => *last_exit_code = oxide_builtins::ps::execute(&expanded_args),
                            "top" => *last_exit_code = oxide_builtins::top::execute(&expanded_args),
                            "echo" => {
                                    let output = expanded_args.join(" "); 
                                    println!("{}", output);
                                    *last_exit_code = 0;
                                }
                            "clear" => *last_exit_code = oxide_builtins::clear::execute(&expanded_args),
                            "jobs" => { job_manager.print_jobs(); *last_exit_code = 0; },
                            "find" => *last_exit_code = oxide_builtins::find::execute(&expanded_args),
                            "touch" => {
                                *last_exit_code = oxide_builtins::touch::execute(&expanded_args);
                                continue;
                            }
                            // Pipeline-specific data commands
                            "open" => {
                                match oxide_builtins::open::get_data(&expanded_args[0]) {
                                    Ok(data) => internal_data = Some(data),
                                    Err(e) => eprintln!("oxide: open: pipeline error: {}", e),
                                }
                            }
                            "get" => {
                                if let Some(data) = internal_data.take() {
                                    internal_data = Some(oxide_builtins::get::execute(&expanded_args, data));
                                } else {
                                    eprintln!("oxide: get: no input data received in pipeline");
                                }
                            }
                            "cat" => {
                                if let Some(data) = internal_data.take() {
                                    internal_data = Some(oxide_builtins::cat::execute_with_input(&expanded_args, data));
                                } else {
                                    eprintln!("oxide: cat: no input data received in pipeline");
                                }
                            }
                            "env" => {
                                if let Some(data) = internal_data.take() {
                                    internal_data = Some(oxide_builtins::env::execute_with_input(data));
                                } else {
                                    internal_data = Some(oxide_builtins::env::execute());
                                }
                            }

                            // OS Fallback Pipeline
                            _ => {
                                let is_last = i == len - 1;
                                
                                // Feed the command into our pipeline manager
                                match os_pipeline.execute_node(&cmd.program, &expanded_args, is_last, &cmd.outfile) {
                                    Ok(Some(code)) => *last_exit_code = code, // The pipeline finished!
                                    Ok(None) => {} // Still running, keep looping!
                                    Err(e) => {
                                        eprintln!("{}", e);
                                        *last_exit_code = 127;
                                        break; 
                                    }
                                }
                            }
                        }
                    }

                    if let Some(final_data) = internal_data {
                        println!("{:#?}", final_data);
                    }
                }
            }
        }
    }
}