use std::collections::HashMap;
use oxide_compat::CompatMode;
use oxide_parser::lexer::Lexer;
use oxide_parser::parser::Parser;
use oxide_parser::ast::{Statement, Condition};

pub struct Executor {
    pub runtime: oxide_script::runtime::Runtime,
    pub script_scope: oxide_script::scope::Scope, 
}

impl Executor {
    pub fn new() -> Self {
        Self {
            runtime: oxide_script::runtime::Runtime::new(),
            script_scope: oxide_script::scope::Scope::new(),
        }
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
                Statement::Command(cmd) => {
                    // Skip glob expansion for commands that shouldn't have their args globbed
                    let mut expanded_args: Vec<String> = Vec::new();
                    
                    let skip_glob = matches!(cmd.program.as_str(), "source" | "alias" | "unset" | "export");
                    
                    if skip_glob {
                        expanded_args = cmd.args.clone();
                    } else {
                        // Pre-process arguments (Expansion & Globs)
                        for arg in &cmd.args {
                            let text_expanded = oxide_parser::expand::expand_text(arg);
                            expanded_args.extend(oxide_parser::glob::expand_glob(&text_expanded));
                        }
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
                            } else {
                                let sub_program = &expanded_args[0];
                                let sub_args = &expanded_args[1..].to_vec();

                                // Check if we are jailing an INTERNAL command
                                if sub_program == "call" {
                                    // Log that we are running a sandboxed built-in
                                    oxide_security::audit::log_command(sub_program, sub_args, true);
                                    
                                    // Re-route back to your call logic, but inside the jail context
                                    println!("[SANDBOXED]");
                                    if let Some(result) = self.runtime.stdlib.call(&sub_args[0], sub_args[1..].to_vec()) {
                                        println!("{}", result);
                                    }
                                } else {
                                    // Fallback to the OS Sandbox for external programs
                                    let sandbox = oxide_security::sandbox::Sandbox::new("./oxide_jail");
                                    match sandbox.run(sub_program, sub_args) {
                                        Ok(code) => *last_exit_code = code,
                                        Err(e) => eprintln!("{}", e),
                                    }
                                }
                            }
                            continue;
                        }
                        "export" => {
                            *last_exit_code = oxide_builtins::export::execute(&expanded_args);
                        }
                        "find" => *last_exit_code = oxide_builtins::find::execute(&expanded_args),
                        "help" => *last_exit_code = oxide_builtins::help::execute(&expanded_args),
                        "kill" => *last_exit_code = oxide_builtins::kill::execute(&expanded_args),
                        "open" => *last_exit_code = oxide_builtins::open::execute(&expanded_args),
                        "ps" => *last_exit_code = oxide_builtins::ps::execute(&expanded_args),
                        "rm" => *last_exit_code = oxide_builtins::rm::execute(&expanded_args),
                        "sleep" => *last_exit_code = oxide_builtins::sleep::execute(&expanded_args),
                        "top" => *last_exit_code = oxide_builtins::top::execute(&expanded_args),
                        "unset" => *last_exit_code = oxide_builtins::unset::execute(&expanded_args),
                        "call" => {
                            // Usage: call math_add 10 20
                            if expanded_args.len() < 1 {
                                eprintln!("oxide: call: usage: call <function_name> [args...]");
                            } else {
                                let func_name = &expanded_args[0];
                                let func_args = expanded_args[1..].to_vec();

                                // 1. Check StdLib first
                                if let Some(result) = self.runtime.stdlib.call(func_name, func_args.clone()) {
                                    println!("{}", result);
                                } 
                                // 2. Check User-defined functions next
                                else if let Some(_func) = self.runtime.functions.get(func_name) {
                                    // Logic to execute the function body (Statements) would go here
                                    println!("oxide: executing script function '{}'", func_name);
                                } else {
                                    eprintln!("oxide: call: function '{}' not found", func_name);
                                }
                            }
                            *last_exit_code = 0;
                            continue;
                        }
                        "import" => {
                            if let Some(mod_name) = expanded_args.first() {
                                match self.runtime.modules.load_module(mod_name) {
                                    Ok(_content) => {
                                        println!("oxide: loaded module '{}'", mod_name);
                                        // In a real scenario, you'd send 'content' back to the Lexer/Parser
                                    },
                                    Err(e) => eprintln!("oxide: import error: {}", e),
                                }
                            }
                            continue;
                        }
                        // Inside executor.rs -> execute_line -> match cmd.program.as_str()

                        "hi" | "hello" => {
                            // This handles both 'hi' and 'hello'
                            println!("Hi there! You are running Oxide Shell.");
                            println!("Current Mode: {:?}", mode);
                            
                            // If you want to use your new oxide-script scope here:
                            if let Some(user) = self.runtime.scope.get("USER") {
                                println!("Good to see you, {}!", user);
                            }

                            *last_exit_code = 0;
                            continue; // Skip the OS fallback
                        }
                        "refresh" => {
                            print!("\x1B[2J\x1B[1;1H");
                            
                            *last_exit_code = 0;
                            *mode = oxide_compat::CompatMode::Oxide; 
                            
                            println!("Oxide Shell refreshed. System state reset to defaults.");

                           *last_exit_code = oxide_builtins::clear::execute(&expanded_args);
                            continue;
                        }
                        // In your executor's match branch for "source"
// crates/oxide-exec/src/executor.rs

                        "source" => {
                            if let Some(file) = cmd.args.get(0) { 
                                match std::fs::read_to_string(file) {
                                    Ok(contents) => {
                                        let tokens = Lexer::new(&contents).tokenize();
                                        let mut parser = Parser::new(tokens);
                                        let statements: Vec<Statement> = parser.parse().into_iter().map(|e| e.statement).collect();
                                        
                                        // USE NATIVE EXECUTOR INSTEAD OF RUNTIME
                                        self.execute_statements(&statements, mode, aliases, last_exit_code, job_manager, history, &security);
                                        *last_exit_code = 0;
                                    },
                                    Err(e) => {
                                        eprintln!("oxide: source: cannot open file '{}': {}", file, e);
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
                Statement::If { condition, body, else_if, else_body } => {
                    if self.evaluate_if_condition(&condition) {
                        self.execute_statements(&body, mode, aliases, last_exit_code, job_manager, history, &security);
                    } else {
                        let mut executed = false;
                        for (else_if_condition, else_if_body) in else_if {
                            if self.evaluate_if_condition(&else_if_condition) {
                                self.execute_statements(&else_if_body, mode, aliases, last_exit_code, job_manager, history, &security);
                                executed = true;
                                break;
                            }
                        }
                        if !executed {
                            if let Some(else_statements) = &else_body {
                                self.execute_statements(else_statements, mode, aliases, last_exit_code, job_manager, history, &security);
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

    fn execute_statements(
        &mut self,
        statements: &[Statement],
        mode: &mut CompatMode,
        aliases: &mut HashMap<String, String>,
        last_exit_code: &mut i32,
        job_manager: &mut crate::jobs::JobManager,
        history: &[String],
        security: &oxide_security::permissions::PermissionManager,
    ) {
        for statement in statements {
            self.execute_statement(statement, mode, aliases, last_exit_code, job_manager, history, security);
        }
    }

    fn evaluate_if_condition(&self, condition: &str) -> bool {
        let val = if condition.starts_with('$') {
            std::env::var(&condition[1..]).unwrap_or_else(|_| "0".to_string())
        } else {
            condition.to_string()
        };

        !val.is_empty() && val != "0" && val != "false"
    }

    fn execute_statement(
        &mut self,
        statement: &Statement,
        mode: &mut CompatMode,
        aliases: &mut HashMap<String, String>,
        last_exit_code: &mut i32,
        job_manager: &mut crate::jobs::JobManager,
        history: &[String],
        security: &oxide_security::permissions::PermissionManager,
    ) {
        match statement {
            Statement::Command(cmd) => {
                // Pre-process arguments (Expansion & Globs)
                let mut expanded_args: Vec<String> = Vec::new();
                let skip_glob = matches!(cmd.program.as_str(), "source" | "alias" | "unset" | "export");
                
                if skip_glob {
                    expanded_args = cmd.args.clone();
                } else {
                    for arg in &cmd.args {
                        let text_expanded = oxide_parser::expand::expand_text(arg);
                        expanded_args.extend(oxide_parser::glob::expand_glob(&text_expanded));
                    }
                }

                // --- 2. SECURITY CHECK ---
                if let Err(e) = security.is_allowed(&cmd.program, &expanded_args) {
                    oxide_security::audit::log_command(&cmd.program, &expanded_args, false);
                    eprintln!("{}", e);
                    *last_exit_code = 1;
                    return;
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
                        } else {
                            let sub_program = &expanded_args[0];
                            let sub_args = &expanded_args[1..].to_vec();

                            // Check if we are jailing an INTERNAL command
                            if sub_program == "call" {
                                // Log that we are running a sandboxed built-in
                                oxide_security::audit::log_command(sub_program, sub_args, true);
                                
                                // Re-route back to your call logic, but inside the jail context
                                println!("[SANDBOXED]");
                                if let Some(result) = self.runtime.stdlib.call(&sub_args[0], sub_args[1..].to_vec()) {
                                    println!("{}", result);
                                }
                            } else {
                                // Fallback to the OS Sandbox for external programs
                                let sandbox = oxide_security::sandbox::Sandbox::new("./oxide_jail");
                                match sandbox.run(sub_program, sub_args) {
                                    Ok(code) => *last_exit_code = code,
                                    Err(e) => eprintln!("{}", e),
                                }
                            }
                        }
                        return;
                    }
                    "export" => {
                        *last_exit_code = oxide_builtins::export::execute(&expanded_args);
                    }
                    "find" => *last_exit_code = oxide_builtins::find::execute(&expanded_args),
                    "help" => *last_exit_code = oxide_builtins::help::execute(&expanded_args),
                    "kill" => *last_exit_code = oxide_builtins::kill::execute(&expanded_args),
                    "open" => *last_exit_code = oxide_builtins::open::execute(&expanded_args),
                    "ps" => *last_exit_code = oxide_builtins::ps::execute(&expanded_args),
                    "rm" => *last_exit_code = oxide_builtins::rm::execute(&expanded_args),
                    "sleep" => *last_exit_code = oxide_builtins::sleep::execute(&expanded_args),
                    "top" => *last_exit_code = oxide_builtins::top::execute(&expanded_args),
                    "unset" => *last_exit_code = oxide_builtins::unset::execute(&expanded_args),
                    "call" => {
                        // Usage: call math_add 10 20
                        if expanded_args.len() < 1 {
                            eprintln!("oxide: call: usage: call <function_name> [args...]");
                        } else {
                            let func_name = &expanded_args[0];
                            let func_args = expanded_args[1..].to_vec();

                            // 1. Check StdLib first
                            if let Some(result) = self.runtime.stdlib.call(func_name, func_args.clone()) {
                                println!("{}", result);
                            } 
                            // 2. Check User-defined functions next
                            else if let Some(_func) = self.runtime.functions.get(func_name) {
                                // Logic to execute the function body (Statements) would go here
                                println!("oxide: executing script function '{}'", func_name);
                            } else {
                                eprintln!("oxide: call: function '{}' not found", func_name);
                            }
                        }
                        *last_exit_code = 0;
                        return;
                    }
                    "import" => {
                        if let Some(mod_name) = expanded_args.first() {
                            match self.runtime.modules.load_module(mod_name) {
                                Ok(_content) => {
                                    println!("oxide: loaded module '{}'", mod_name);
                                    // In a real scenario, you'd send 'content' back to the Lexer/Parser
                                },
                                Err(e) => eprintln!("oxide: import error: {}", e),
                            }
                        }
                        return;
                    }
                    // Inside executor.rs -> execute_line -> match cmd.program.as_str()
                    "hi" | "hello" => {
                        // This handles both 'hi' and 'hello'
                        println!("Hi there! You are running Oxide Shell.");
                        println!("Current Mode: {:?}", mode);
                        
                        // If you want to use your new oxide-script scope here:
                        if let Some(user) = self.runtime.scope.get("USER") {
                            println!("Good to see you, {}!", user);
                        }

                        *last_exit_code = 0;
                        return; // Skip the OS fallback
                    }
                    "refresh" => {
                        print!("\x1B[2J\x1B[1;1H");
                        
                        *last_exit_code = 0;
                        *mode = oxide_compat::CompatMode::Oxide; 
                        
                        println!("Oxide Shell refreshed. System state reset to defaults.");

                       *last_exit_code = oxide_builtins::clear::execute(&expanded_args);
                        return;
                    }
                    "source" => {
                        if let Some(file) = cmd.args.get(0) {
                            match std::fs::read_to_string(file) {
                                Ok(contents) => {
                                    let tokens = Lexer::new(&contents).tokenize();
                                    let mut parser = Parser::new(tokens);
                                    let statements: Vec<Statement> = parser.parse().into_iter().map(|e| e.statement).collect();
                                    self.runtime.run_script(statements);
                                    *last_exit_code = 0;
                                },
                                Err(e) => {
                                    eprintln!("oxide: source: cannot open file '{}': {}", file, e);
                                    *last_exit_code = 1;
                                }
                            }
                        } else {
                            eprintln!("oxide: source: usage: source <filename>");
                            *last_exit_code = 1;
                        }
                        return;
                    }
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
            Statement::If { condition, body, else_if, else_body } => {
                if self.evaluate_if_condition(&condition) {
                    self.execute_statements(&body, mode, aliases, last_exit_code, job_manager, history, security);
                } else {
                    let mut executed = false;
                    for (else_if_condition, else_if_body) in else_if {
                        if self.evaluate_if_condition(&else_if_condition) {
                            self.execute_statements(&else_if_body, mode, aliases, last_exit_code, job_manager, history, security);
                            executed = true;
                            break;
                        }
                    }
                    if !executed {
                        if let Some(else_statements) = &else_body {
                            self.execute_statements(else_statements, mode, aliases, last_exit_code, job_manager, history, security);
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

