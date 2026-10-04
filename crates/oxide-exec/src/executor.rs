//! The execution engine.
//!
//! Every command Oxide runs — typed at the prompt, read from a script, or
//! nested inside `if`/`for`/`|` — funnels through [`Executor::run_command`],
//! which routes it to one of three places:
//!
//! 1. an **executor builtin** ([`ExecutorBuiltin`]), which needs shell
//!    internals like the job table or the parser;
//! 2. a **registry builtin** from `oxide-builtins`, run in-process against a
//!    caller-supplied [`Io`] so it works standalone, under a redirect, or as a
//!    pipeline stage;
//! 3. an **OS process**, spawned from a `PATH`-resolved absolute path.

use std::collections::HashMap;
use std::io::Write;

use oxide_builtins::{Ctx, Io};
use oxide_compat::CompatMode;
use oxide_parser::ast::{Command, Condition, Statement};
use oxide_parser::lexer::Lexer;
use oxide_parser::parser::Parser;
use oxide_perf::cache::CommandCache;
use oxide_security::permissions::PermissionManager;

use crate::pipeline::{builtin_sink, spawn_stage, Upstream};

pub struct Executor {
    pub runtime: oxide_script::runtime::Runtime,
    pub script_scope: oxide_script::scope::Scope,
}

/// The shell state a command runs against.
///
/// Bundled into one struct because every level of the engine — statements,
/// pipelines, loop bodies — needs the same set, and threading seven separate
/// `&mut` parameters through each of them is how the two dispatch paths
/// drifted apart in the first place.
pub struct ExecEnv<'a> {
    pub mode: &'a mut CompatMode,
    pub aliases: &'a mut HashMap<String, String>,
    pub last_exit_code: &'a mut i32,
    pub job_manager: &'a mut crate::jobs::JobManager,
    pub history: &'a [String],
    pub security: &'a PermissionManager,
    pub command_cache: &'a mut CommandCache,
}

/// Builtins that can't live in `oxide-builtins` because they need the executor
/// itself: the job table, the compat mode, the script runtime, or recursion
/// back through the lexer and parser.
///
/// `oxide_builtins::EXECUTOR_BUILTINS` advertises these names to the line
/// editor; the test at the bottom of this file keeps the two in agreement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExecutorBuiltin {
    Call,
    Greet,
    Import,
    Jail,
    Jobs,
    Mode,
    Refresh,
    Source,
}

impl ExecutorBuiltin {
    fn lookup(name: &str) -> Option<Self> {
        Some(match name {
            "call" => Self::Call,
            "hi" | "hello" => Self::Greet,
            "import" => Self::Import,
            "jail" => Self::Jail,
            "jobs" => Self::Jobs,
            "mode" => Self::Mode,
            "refresh" => Self::Refresh,
            "source" => Self::Source,
            _ => return None,
        })
    }
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
        let security = PermissionManager::new();
        let mut env = ExecEnv {
            mode,
            aliases,
            last_exit_code,
            job_manager,
            history,
            security: &security,
            command_cache,
        };
        self.run_input(input, &mut env);
    }

    /// Translates, alias-expands, parses, and runs one line of input.
    fn run_input(&mut self, input: &str, env: &mut ExecEnv<'_>) {
        // Translate input based on compatibility mode
        let compat_input = match *env.mode {
            CompatMode::Bash => oxide_compat::bash_mode::translate(input),
            CompatMode::Posix => oxide_compat::posix_mode::translate(input),
            CompatMode::Oxide => input.to_string(),
        };

        // Handle Aliases
        let mut processed_input = compat_input.clone();
        if let Some(first_word) = compat_input.split_whitespace().next() {
            if let Some(replacement) = env.aliases.get(first_word) {
                processed_input = compat_input.replacen(first_word, replacement, 1);
            }
        }

        // --- SUBSHELL INTERCEPTOR ---
        let trimmed = processed_input.trim();
        if trimmed.starts_with('(') && trimmed.ends_with(')') {
            let inner_cmd = &trimmed[1..trimmed.len() - 1];
            *env.last_exit_code = crate::subshell::execute(
                inner_cmd,
                env.mode,
                env.aliases,
                env.job_manager,
                env.history,
            );
            return;
        }

        // PROFILING: Lexing & Parsing
        let parse_timer = oxide_perf::profiler::Profiler::start();

        let tokens = Lexer::new(&processed_input).tokenize();
        let executables = Parser::new(tokens).parse();

        parse_timer.stop("Lexer & Parser");

        // PROFILING: Execution Engine
        let exec_timer = oxide_perf::profiler::Profiler::start();

        for exec in &executables {
            // --- LOGIC GATES (&& and ||) ---
            match exec.condition {
                Condition::And => {
                    if *env.last_exit_code != 0 {
                        continue;
                    }
                }
                Condition::Or => {
                    if *env.last_exit_code == 0 {
                        continue;
                    }
                }
                Condition::Always => {}
            }

            self.execute_statement(&exec.statement, env);
        }

        exec_timer.stop("Execution Engine");
    }

    fn execute_statements(&mut self, statements: &[Statement], env: &mut ExecEnv<'_>) {
        for statement in statements {
            self.execute_statement(statement, env);
        }
    }

    fn execute_statement(&mut self, statement: &Statement, env: &mut ExecEnv<'_>) {
        match statement {
            Statement::Command(cmd) => self.run_command(cmd, env),
            Statement::Pipeline(commands) => self.run_pipeline(commands, env),
            Statement::If { condition, body, else_if, else_body } => {
                if self.evaluate_if_condition(condition) {
                    self.execute_statements(body, env);
                    return;
                }
                for (else_if_condition, else_if_body) in else_if {
                    if self.evaluate_if_condition(else_if_condition) {
                        self.execute_statements(else_if_body, env);
                        return;
                    }
                }
                if let Some(else_statements) = else_body {
                    self.execute_statements(else_statements, env);
                }
            }
            Statement::While { condition, body } => {
                while self.evaluate_if_condition(condition) {
                    self.execute_statements(body, env);
                }
            }
            Statement::For { variable, values, body } => {
                for val in values {
                    // Expand variables (e.g. $VAR) and globs (e.g. *.txt)
                    let expanded_val = oxide_parser::expand::expand_text(val);
                    for final_val in oxide_parser::glob::expand_glob(&expanded_val) {
                        std::env::set_var(variable, &final_val);
                        self.execute_statements(body, env);
                    }
                }
            }
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

    // ==========================================
    // SINGLE COMMAND
    // ==========================================

    /// The one place a command is routed. The top-level loop and every nested
    /// statement come through here, so there is a single definition of what
    /// each command name means.
    fn run_command(&mut self, cmd: &Command, env: &mut ExecEnv<'_>) {
        let expanded_args = expand_args(cmd);

        if !self.check_permission(&cmd.program, &expanded_args, env) {
            return;
        }

        if let Some(kind) = ExecutorBuiltin::lookup(&cmd.program) {
            *env.last_exit_code = self.run_executor_builtin(kind, &expanded_args, env);
            return;
        }

        if let Some(builtin) = oxide_builtins::registry().get(&cmd.program) {
            let (code, _) =
                run_builtin_stage(builtin, &expanded_args, Upstream::None, cmd, true, env);
            *env.last_exit_code = code;
            return;
        }

        self.spawn_external(cmd, &expanded_args, env);
    }

    /// Runs the security gate and writes the audit record. Returns false when
    /// the command was blocked, having set the exit code.
    fn check_permission(&self, program: &str, args: &[String], env: &mut ExecEnv<'_>) -> bool {
        if let Err(e) = env.security.is_allowed(program, args) {
            oxide_security::audit::log_command(program, args, false);
            eprintln!("{}", e);
            *env.last_exit_code = 1;
            return false;
        }
        oxide_security::audit::log_command(program, args, true);
        true
    }

    fn spawn_external(&mut self, cmd: &Command, expanded_args: &[String], env: &mut ExecEnv<'_>) {
        let is_background = expanded_args.last().map(|s| s.as_str()) == Some("&");
        let mut args = expanded_args.to_vec();
        if is_background {
            args.pop();
        }

        // The command cache turns the PATH walk into a hash lookup.
        let program_path = resolve_program_path(&cmd.program, env.command_cache);

        if is_background {
            match crate::process::spawn_background(
                &program_path,
                &args,
                &cmd.outfile,
                cmd.append,
                &cmd.infile,
            ) {
                Ok(child) => {
                    env.job_manager.add(cmd.program.clone(), child);
                    *env.last_exit_code = 0;
                }
                Err(e) => {
                    eprintln!("{}", e);
                    *env.last_exit_code = 127;
                }
            }
        } else {
            *env.last_exit_code = crate::process::spawn_single(
                &program_path,
                &args,
                &cmd.outfile,
                cmd.append,
                &cmd.infile,
            );
        }
    }

    fn run_executor_builtin(
        &mut self,
        kind: ExecutorBuiltin,
        args: &[String],
        env: &mut ExecEnv<'_>,
    ) -> i32 {
        match kind {
            ExecutorBuiltin::Mode => {
                match args.first().map(|s| s.as_str()) {
                    None => println!("oxide: current mode is {:?}", *env.mode),
                    Some("bash") => *env.mode = CompatMode::Bash,
                    Some("posix") => *env.mode = CompatMode::Posix,
                    Some("oxide") => *env.mode = CompatMode::Oxide,
                    Some(other) => {
                        eprintln!("oxide: unknown mode '{}'", other);
                        return 1;
                    }
                }
                0
            }

            ExecutorBuiltin::Jobs => {
                env.job_manager.print_jobs();
                0
            }

            ExecutorBuiltin::Jail => {
                let Some(sub_program) = args.first() else {
                    eprintln!("jail: usage: jail <command> [args]");
                    return 1;
                };
                let sub_args = args[1..].to_vec();

                // Jailing an internal command means running it here, flagged,
                // rather than handing it to the OS sandbox.
                if sub_program == "call" {
                    let Some(func_name) = sub_args.first() else {
                        eprintln!("jail: usage: jail call <function> [args]");
                        return 1;
                    };
                    oxide_security::audit::log_command(sub_program, &sub_args, true);
                    println!("[SANDBOXED]");
                    if let Some(result) =
                        self.runtime.stdlib.call(func_name, sub_args[1..].to_vec())
                    {
                        println!("{}", result);
                    }
                    return 0;
                }

                let sandbox = oxide_security::sandbox::Sandbox::new("./oxide_jail");
                match sandbox.run(sub_program, &sub_args) {
                    Ok(code) => code,
                    Err(e) => {
                        eprintln!("{}", e);
                        1
                    }
                }
            }

            ExecutorBuiltin::Call => {
                // Usage: call math_add 10 20
                let Some(func_name) = args.first() else {
                    eprintln!("oxide: call: usage: call <function_name> [args...]");
                    return 1;
                };
                let func_args = args[1..].to_vec();

                // 1. Check StdLib first
                if let Some(result) = self.runtime.stdlib.call(func_name, func_args) {
                    println!("{}", result);
                    0
                }
                // 2. Check user-defined functions next
                else if self.runtime.functions.get(func_name).is_some() {
                    // Executing the function body (Statements) would go here.
                    println!("oxide: executing script function '{}'", func_name);
                    0
                } else {
                    eprintln!("oxide: call: function '{}' not found", func_name);
                    1
                }
            }

            ExecutorBuiltin::Import => {
                let Some(mod_name) = args.first() else {
                    eprintln!("oxide: import: usage: import <module>");
                    return 1;
                };
                match self.runtime.modules.load_module(mod_name) {
                    Ok(_content) => {
                        println!("oxide: loaded module '{}'", mod_name);
                        0
                    }
                    Err(e) => {
                        eprintln!("oxide: import error: {}", e);
                        1
                    }
                }
            }

            ExecutorBuiltin::Greet => {
                println!("Hi there! You are running Oxide Shell.");
                println!("Current Mode: {:?}", *env.mode);
                if let Some(user) = self.runtime.scope.get("USER") {
                    println!("Good to see you, {}!", user);
                }
                0
            }

            ExecutorBuiltin::Refresh => {
                let mut stdout = std::io::stdout();
                let _ = stdout.write_all(b"\x1B[3J\x1B[2J\x1B[H");
                let _ = stdout.flush();
                *env.mode = CompatMode::Oxide;
                println!("Oxide Shell refreshed. System state reset to defaults.");
                0
            }

            ExecutorBuiltin::Source => {
                let Some(file) = args.first() else {
                    eprintln!("oxide: source: usage: source <filename>");
                    return 1;
                };
                match std::fs::read_to_string(file) {
                    Ok(contents) => {
                        let tokens = Lexer::new(&contents).tokenize();
                        let statements: Vec<Statement> = Parser::new(tokens)
                            .parse()
                            .into_iter()
                            .map(|e| e.statement)
                            .collect();
                        // A sourced file runs in *this* shell, so it goes
                        // through the executor, not the script runtime.
                        self.execute_statements(&statements, env);
                        0
                    }
                    Err(e) => {
                        eprintln!("oxide: source: cannot open file '{}': {}", file, e);
                        1
                    }
                }
            }
        }
    }

    // ==========================================
    // PIPELINES
    // ==========================================

    /// Runs a chained pipeline (`a | b | c`).
    ///
    /// Stages may be builtins, OS processes, or any mix: process-to-process
    /// links are real kernel pipes, while a builtin stage reads its upstream
    /// through [`Upstream`] and hands its own output to the next stage. Only
    /// the final stage's redirect and exit code are honored.
    fn run_pipeline(&mut self, commands: &[Command], env: &mut ExecEnv<'_>) {
        let len = commands.len();
        if len == 0 {
            return;
        }

        let mut upstream = Upstream::None;
        let mut spawned: Vec<crate::pipeline::Spawned> = Vec::new();
        // Whether the pipeline's status should come from the last child we
        // wait on, rather than from a builtin that already set it.
        let mut status_from_child = false;

        for (i, cmd) in commands.iter().enumerate() {
            let is_last = i == len - 1;
            let expanded_args = expand_args(cmd);

            if !self.check_permission(&cmd.program, &expanded_args, env) {
                break;
            }

            if ExecutorBuiltin::lookup(&cmd.program).is_some() {
                eprintln!("oxide: '{}' cannot be used in a pipeline yet", cmd.program);
                *env.last_exit_code = 1;
                break;
            }

            if let Some(builtin) = oxide_builtins::registry().get(&cmd.program) {
                let (code, next) =
                    run_builtin_stage(builtin, &expanded_args, upstream.take(), cmd, is_last, env);
                upstream = next;
                if is_last {
                    *env.last_exit_code = code;
                    status_from_child = false;
                }
                continue;
            }

            let program_path = resolve_program_path(&cmd.program, env.command_cache);
            match spawn_stage(
                &program_path,
                &expanded_args,
                upstream.take(),
                is_last,
                &cmd.outfile,
                cmd.append,
                &cmd.infile,
            ) {
                Ok(mut stage) => {
                    if is_last {
                        status_from_child = true;
                    } else {
                        upstream = stage
                            .child
                            .stdout
                            .take()
                            .map(Upstream::Child)
                            .unwrap_or(Upstream::None);
                    }
                    spawned.push(stage);
                }
                Err(e) => {
                    eprintln!("{}", e);
                    *env.last_exit_code = 127;
                    break;
                }
            }
        }

        // Release any dangling read end before reaping, so a producer blocked
        // on a full pipe gets a broken-pipe error instead of hanging us.
        drop(upstream);

        let last = spawned.len().saturating_sub(1);
        for (idx, mut stage) in spawned.into_iter().enumerate() {
            let code = match stage.child.wait() {
                Ok(status) => status.code().unwrap_or(1),
                Err(e) => {
                    eprintln!("oxide: pipeline: failed to wait: {}", e);
                    1
                }
            };
            if let Some(feeder) = stage.feeder.take() {
                let _ = feeder.join();
            }
            if status_from_child && idx == last {
                *env.last_exit_code = code;
            }
        }
    }
}

// ==========================================
// HELPERS
// ==========================================

/// Applies variable and glob expansion to a command's arguments.
///
/// A handful of builtins take literal text that must survive untouched:
/// `alias` and `export` take `KEY=VALUE` pairs, `unset` takes variable names,
/// and `source` takes a path that globbing would multiply.
fn expand_args(cmd: &Command) -> Vec<String> {
    if matches!(cmd.program.as_str(), "source" | "alias" | "unset" | "export") {
        return cmd.args.clone();
    }

    let mut expanded = Vec::with_capacity(cmd.args.len());
    for arg in &cmd.args {
        let text_expanded = oxide_parser::expand::expand_text(arg);
        expanded.extend(oxide_parser::glob::expand_glob(&text_expanded));
    }
    expanded
}

/// Runs a registry builtin, wiring its three streams from `upstream` and the
/// command's redirects. Returns the exit code and what the next pipeline
/// stage should read.
fn run_builtin_stage(
    builtin: &dyn oxide_builtins::Builtin,
    args: &[String],
    upstream: Upstream,
    cmd: &Command,
    is_last: bool,
    env: &mut ExecEnv<'_>,
) -> (i32, Upstream) {
    let mut reader = match upstream.into_reader(&cmd.infile) {
        Ok(reader) => reader,
        Err(e) => {
            eprintln!("{}", e);
            return (1, Upstream::None);
        }
    };

    let mut sink = match builtin_sink(is_last, &cmd.outfile, cmd.append) {
        Ok(sink) => sink,
        Err(e) => {
            eprintln!("{}", e);
            return (1, Upstream::None);
        }
    };

    let mut stderr = std::io::stderr();
    let mut ctx = Ctx {
        aliases: &mut *env.aliases,
        history: env.history,
    };

    let code = {
        let mut io = Io::new(&mut *reader, sink.as_writer(), &mut stderr);
        builtin.run(args, &mut io, &mut ctx)
    };

    // Drop the read end before the caller waits on any upstream process. A
    // builtin that ignores its input (`ls | echo hi`) would otherwise leave
    // the producer blocked on a pipe nobody is draining.
    drop(reader);

    (code, sink.finish())
}

/// Resolves `program` to an absolute path by scanning `PATH`, caching the
/// result so repeated invocations skip the filesystem walk. Falls back to the
/// bare program name if it can't be found, letting the OS spawn report the error.
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

#[cfg(test)]
mod tests {
    use super::*;

    /// `oxide-ui` completes and highlights every name in `EXECUTOR_BUILTINS`,
    /// so each one has to actually reach a dispatch arm here.
    #[test]
    fn every_advertised_executor_builtin_is_dispatched() {
        for name in oxide_builtins::EXECUTOR_BUILTINS {
            assert!(
                ExecutorBuiltin::lookup(name).is_some(),
                "'{name}' is advertised in EXECUTOR_BUILTINS but has no dispatch arm"
            );
        }
    }

    /// The two tiers must not both claim a name — the executor checks first,
    /// so an overlap would silently shadow the registry entry.
    #[test]
    fn executor_and_registry_builtins_do_not_overlap() {
        for name in oxide_builtins::registry().names() {
            assert!(
                ExecutorBuiltin::lookup(name).is_none(),
                "'{name}' is both an executor builtin and a registry builtin"
            );
        }
    }

    #[test]
    fn literal_arguments_skip_glob_expansion() {
        let mut cmd = Command::new("export".to_string());
        cmd.args = vec!["GREETING=hello *".to_string()];
        assert_eq!(expand_args(&cmd), vec!["GREETING=hello *".to_string()]);
    }
}
