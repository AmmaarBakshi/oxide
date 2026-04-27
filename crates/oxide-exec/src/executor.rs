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

        let mut lexer = Lexer::new(&processed_input);
        let tokens = lexer.tokenize();
        let mut parser = Parser::new(tokens);
        let executables = parser.parse();

        for exec in executables {
            match exec.condition {
                Condition::And if *last_exit_code != 0 => continue,
                Condition::Or if *last_exit_code == 0 => continue,
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
                        *last_exit_code = oxide_builtins::echo::execute(&expanded_args, &cmd.outfile);
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
                    let mut previous_stdout = None;
                    let len = commands.len();
                    let mut internal_data: Option<oxide_data::value::Value> = None;

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
                            *last_exit_code = oxide_builtins::pwd::execute(&expanded_args);
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
                            match oxide_builtins::open::get_data(&expanded_args[0]) {
                                Ok(data) => {
                                    internal_data = Some(data);
                                }
                                Err(e) => {
                                    eprintln!("oxide: open: pipeline error: {}", e);
                                }
                            }
                            continue;
                        } else if cmd.program == "get" {
                            if let Some(data) = internal_data.take() {
                                internal_data = Some(oxide_builtins::get::execute(&expanded_args, data));
                            } else {
                                eprintln!("oxide: get: no input data received in pipeline");
                            }
                            continue;
                        } else if cmd.program == "echo" {
                            *last_exit_code = oxide_builtins::echo::execute(&expanded_args, &cmd.outfile);
                            continue;
                        } else if cmd.program == "clear" {
                            *last_exit_code = oxide_builtins::clear::execute(&expanded_args);
                            continue;
                        } else if cmd.program == "jobs" {
                            job_manager.print_jobs();
                            *last_exit_code = 0;
                            continue;
                        } else if cmd.program == "find" {
                            *last_exit_code = oxide_builtins::find::execute(&expanded_args);
                            continue;
                        }
                        // --- OS FALLBACK ---
                        let is_last = i == len - 1;
                        match crate::process::spawn_piped(
                            &cmd.program, 
                            &expanded_args, 
                            previous_stdout.take(), 
                            is_last, 
                            &cmd.outfile
                        ) {
                            Ok(mut child) => {
                                if !is_last { 
                                    previous_stdout = child.stdout.take(); 
                                } else {
                                    let status = child.wait().expect("failed to wait");
                                    *last_exit_code = status.code().unwrap_or(1);
                                }
                            }
                            Err(err_msg) => {
                                eprintln!("{}", err_msg);
                                *last_exit_code = 127;
                                break; 
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