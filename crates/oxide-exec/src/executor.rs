use std::collections::HashMap;
use oxide_compat::CompatMode;
use oxide_parser::lexer::Lexer;
use oxide_parser::parser::Parser;
use oxide_parser::ast::{Statement, Condition, Command};
use oxide_perf::cache::CommandCache;

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
        history: &[String],
        command_cache: &mut CommandCache,
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

        // ==========================================
        // ⏱️ PROFILING START: Lexing & Parsing
        // ==========================================
        let parse_timer = oxide_perf::profiler::Profiler::start();

        // Tokenize and Parse
        let mut lexer = Lexer::new(&processed_input);
        let tokens = lexer.tokenize();
        let mut parser = Parser::new(tokens);
        let executables = parser.parse();

        parse_timer.stop("Lexer & Parser");

        // ==========================================
        // ⏱️ PROFILING START: Execution Engine
        // ==========================================
        let exec_timer = oxide_perf::profiler::Profiler::start();

        for exec in executables {
            // --- LOGIC GATES (&& and ||) ---
            match exec.condition {
                Condition::And => if *last_exit_code != 0 { continue; },
                Condition::Or => if *last_exit_code == 0 { continue; },
                Condition::Always => {}
            }

            match exec.statement {
                Statement::Command(cmd) => {
                    // ... [Your existing Command matching logic stays exactly the same here] ...
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
                                use std::io::Write;
                                let opened = if cmd.append {
                                    std::fs::OpenOptions::new().create(true).append(true).open(filename)
                                } else {
                                    std::fs::File::create(filename)
                                };
                                if let Ok(mut file) = opened {
                                    let _ = writeln!(file, "{}", output);
                                }
                            } else {
                                println!("{}", output);
                            }
                            *last_exit_code = 0;
                        }
                        "touch" => *last_exit_code = oxide_builtins::touch::execute(&expanded_args),
                        "mkdir" => *last_exit_code = oxide_builtins::mkdir::execute(&expanded_args),
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
                                        self.execute_statements(
                                            &statements, mode, aliases, last_exit_code,
                                            job_manager, history, &security, command_cache
                                        );
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

                            // ==========================================
                            // ⚡ THE SPEED BOOST: Command Cache Check
                            // ==========================================
                            let program_path = resolve_program_path(&cmd.program, command_cache);

                            if is_background {
                                match crate::process::spawn_background(&program_path, &args, &cmd.outfile, cmd.append, &cmd.infile) {
                                    Ok(child) => {
                                        job_manager.add(cmd.program.clone(), child);
                                        *last_exit_code = 0;
                                    }
                                    Err(e) => { eprintln!("{}", e); *last_exit_code = 127; }
                                }
                            } else {
                                // Use the cached absolute path instead of just the name!
                                *last_exit_code = crate::process::spawn_single(&program_path, &args, &cmd.outfile, cmd.append, &cmd.infile);
                            }
                        }
                    }
                }
                Statement::If { condition, body, else_if, else_body } => {
                    if self.evaluate_if_condition(&condition) {
                        self.execute_statements(&body, mode, aliases, last_exit_code, job_manager, history, &security, command_cache);
                    } else {
                        let mut executed = false;
                        for (else_if_condition, else_if_body) in else_if {
                            if self.evaluate_if_condition(&else_if_condition) {
                                self.execute_statements(&else_if_body, mode, aliases, last_exit_code, job_manager, history, &security, command_cache);
                                executed = true;
                                break;
                            }
                        }
                        if !executed {
                            if let Some(else_statements) = &else_body {
                                self.execute_statements(else_statements, mode, aliases, last_exit_code, job_manager, history, &security, command_cache);
                            }
                        }
                    }
                }
                Statement::Pipeline(commands) => {
                    self.execute_pipeline(&commands, last_exit_code, command_cache, &security);
                }
                Statement::While { condition, body } => {
                    while self.evaluate_if_condition(&condition) {
                        for b_stmt in &body {
                            self.execute_statement(b_stmt, mode, aliases, last_exit_code, job_manager, history, &security, command_cache);
                        }
                    }
                }
                Statement::For { variable, values, body } => {
                    for val in values {
                        // Expand variables (e.g. $VAR) and globs (e.g. *.txt)
                        let expanded_val = oxide_parser::expand::expand_text(&val);
                        let final_values = oxide_parser::glob::expand_glob(&expanded_val);
                        
                        for final_val in final_values {
                            // Set the iteration variable (e.g., set i=1)
                            std::env::set_var(&variable, &final_val);
                            
                            // Run the body
                            for b_stmt in &body {
                                self.execute_statement(b_stmt, mode, aliases, last_exit_code, job_manager, history, &security, command_cache);
                            }
                        }
                    }
                }
            }
        }

        exec_timer.stop("Execution Engine");
    }

    /// Runs a chained pipeline (`a | b | c`), streaming each command's stdout
    /// into the next command's stdin. Only the final command's `outfile`
    /// redirect and exit code are honored.
    ///
    /// Note: pipeline stages are spawned as external OS processes, so shell
    /// builtins (e.g. `grep`, `cat`) don't participate here yet — that needs
    /// builtins refactored to read stdin / write to a captured stdout.
    fn execute_pipeline(
        &mut self,
        commands: &[Command],
        last_exit_code: &mut i32,
        command_cache: &mut CommandCache,
        security: &oxide_security::permissions::PermissionManager,
    ) {
        let mut pipeline = crate::pipeline::OsPipeline::new();
        let len = commands.len();

        for (i, cmd) in commands.iter().enumerate() {
            let is_last = i == len - 1;

            // Expand arguments (env vars + globs), matching single-command behavior.
            let mut expanded_args: Vec<String> = Vec::new();
            for arg in &cmd.args {
                let text_expanded = oxide_parser::expand::expand_text(arg);
                expanded_args.extend(oxide_parser::glob::expand_glob(&text_expanded));
            }

            // --- SECURITY CHECK ---
            if let Err(e) = security.is_allowed(&cmd.program, &expanded_args) {
                oxide_security::audit::log_command(&cmd.program, &expanded_args, false);
                eprintln!("{}", e);
                *last_exit_code = 1;
                return;
            }
            oxide_security::audit::log_command(&cmd.program, &expanded_args, true);

            let program_path = resolve_program_path(&cmd.program, command_cache);

            match pipeline.execute_node(&program_path, &expanded_args, is_last, &cmd.outfile, cmd.append, &cmd.infile) {
                Ok(Some(code)) => *last_exit_code = code,
                Ok(None) => {} // Intermediate stage; keep chaining.
                Err(e) => {
                    eprintln!("{}", e);
                    *last_exit_code = 127;
                    return;
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
        command_cache: &mut CommandCache,
    ) {
        for statement in statements {
            self.execute_statement(statement, mode, aliases, last_exit_code, job_manager, history, security, command_cache);
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
        command_cache: &mut CommandCache,
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
                            use std::io::Write;
                            let opened = if cmd.append {
                                std::fs::OpenOptions::new().create(true).append(true).open(filename)
                            } else {
                                std::fs::File::create(filename)
                            };
                            if let Ok(mut file) = opened {
                                let _ = writeln!(file, "{}", output);
                            }
                        } else {
                            println!("{}", output);
                        }
                        *last_exit_code = 0;
                    }
                    "touch" => *last_exit_code = oxide_builtins::touch::execute(&expanded_args),
                    "mkdir" => *last_exit_code = oxide_builtins::mkdir::execute(&expanded_args),
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

                        let program_path = resolve_program_path(&cmd.program, command_cache);

                        if is_background {
                            match crate::process::spawn_background(&program_path, &args, &cmd.outfile, cmd.append, &cmd.infile) {
                                Ok(child) => {
                                    job_manager.add(cmd.program.clone(), child);
                                    *last_exit_code = 0;
                                }
                                Err(e) => { eprintln!("{}", e); *last_exit_code = 127; }
                            }
                        } else {
                            *last_exit_code = crate::process::spawn_single(&program_path, &args, &cmd.outfile, cmd.append, &cmd.infile);
                        }
                    }
                }
            }
            Statement::If { condition, body, else_if, else_body } => {
                if self.evaluate_if_condition(&condition) {
                    self.execute_statements(&body, mode, aliases, last_exit_code, job_manager, history, security, command_cache);
                } else {
                    let mut executed = false;
                    for (else_if_condition, else_if_body) in else_if {
                        if self.evaluate_if_condition(&else_if_condition) {
                            self.execute_statements(&else_if_body, mode, aliases, last_exit_code, job_manager, history, security, command_cache);
                            executed = true;
                            break;
                        }
                    }
                    if !executed {
                        if let Some(else_statements) = &else_body {
                            self.execute_statements(else_statements, mode, aliases, last_exit_code, job_manager, history, security, command_cache);
                        }
                    }
                }
            }
            Statement::Pipeline(commands) => {
                self.execute_pipeline(commands, last_exit_code, command_cache, security);
            }
            Statement::While { condition, body } => {
                while self.evaluate_if_condition(condition) {
                    for b_stmt in body {
                        self.execute_statement(b_stmt, mode, aliases, last_exit_code, job_manager, history, security, command_cache);
                    }
                }
            }
            Statement::For { variable, values, body } => {
                for val in values {
                    let expanded_val = oxide_parser::expand::expand_text(val);
                    let final_values = oxide_parser::glob::expand_glob(&expanded_val);
                    
                    for final_val in final_values {
                        std::env::set_var(variable, &final_val);
                        
                        for b_stmt in body {
                            self.execute_statement(b_stmt, mode, aliases, last_exit_code, job_manager, history, security, command_cache);
                        }
                    }
                }
            }
        }
    }
}

/// Resolves `program` to an absolute path by scanning `PATH`, caching the
/// result so repeated invocations skip the filesystem walk. Falls back to the
/// bare program name if it can't be found (letting the OS spawn report the error).
fn resolve_program_path(program: &str, command_cache: &mut CommandCache) -> String {
    if let Some(cached_path) = command_cache.get(program) {
        return cached_path.to_string_lossy().to_string();
    }

    if let Ok(paths) = std::env::var("PATH") {
        for dir in std::env::split_paths(&paths) {
            let full_path = dir.join(program);
            let full_path_exe = dir.join(format!("{}.exe", program)); // Windows compat

            if full_path.is_file() {
                let resolved = full_path.to_string_lossy().to_string();
                command_cache.insert(program.to_string(), full_path);
                return resolved;
            } else if full_path_exe.is_file() {
                let resolved = full_path_exe.to_string_lossy().to_string();
                command_cache.insert(program.to_string(), full_path_exe);
                return resolved;
            }
        }
    }

    program.to_string()
}

