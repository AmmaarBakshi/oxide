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
use std::ffi::OsString;
use std::io::Write;

use oxide_builtins::{Ctx, Io};
use oxide_compat::CompatMode;
use oxide_parser::ast::{Assignment, Command, Condition, Statement};
use oxide_parser::expand::{expand_string, expand_word, Context};
use oxide_parser::lexer::Lexer;
use oxide_parser::parser::Parser;
use oxide_perf::cache::CommandCache;
use oxide_security::permissions::PermissionManager;

use crate::pipeline::{builtin_sink, spawn_stage, Upstream};

pub struct Executor {
    pub runtime: oxide_script::runtime::Runtime,
    pub script_scope: oxide_script::scope::Scope,
    /// Set by `exit`: the status the shell should exit with. Once it is set
    /// nothing else runs, however deeply nested the `exit` was.
    pub exit_status: Option<i32>,
    /// Shell variables (`NAME=value`). Exported ones live in the process
    /// environment instead, where child processes can see them.
    pub vars: HashMap<String, String>,
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
    Exit,
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
            "exit" => Self::Exit,
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

/// What `$` expansions read: the shell's own state, then the environment.
struct ShellContext<'s> {
    last_status: i32,
    vars: &'s HashMap<String, String>,
}

impl Context for ShellContext<'_> {
    fn lookup(&self, name: &str) -> Option<String> {
        match name {
            "?" => Some(self.last_status.to_string()),
            _ => self
                .vars
                .get(name)
                .cloned()
                .or_else(|| std::env::var(name).ok()),
        }
    }
}

impl Default for Executor {
    fn default() -> Self {
        Self::new()
    }
}

impl Executor {
    pub fn new() -> Self {
        Self {
            runtime: oxide_script::runtime::Runtime::new(),
            script_scope: oxide_script::scope::Scope::new(),
            exit_status: None,
            vars: HashMap::new(),
        }
    }

    /// The expansion context for the current state.
    fn context<'s>(&'s self, env: &ExecEnv<'_>) -> ShellContext<'s> {
        ShellContext {
            last_status: *env.last_exit_code,
            vars: &self.vars,
        }
    }

    /// Sets a variable: in the environment if it is exported there,
    /// otherwise as a shell variable.
    fn set_var(&mut self, name: &str, value: String) {
        if std::env::var_os(name).is_some() {
            std::env::set_var(name, value);
        } else {
            self.vars.insert(name.to_string(), value);
        }
    }

    // The parameters are bundled into an `ExecEnv` straight away; callers
    // hold them as separate fields of their own state.
    #[allow(clippy::too_many_arguments)]
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
                &self.vars,
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
            if self.exit_status.is_some() {
                break;
            }

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
            if self.exit_status.is_some() {
                return;
            }
            self.execute_statement(statement, env);
        }
    }

    fn execute_statement(&mut self, statement: &Statement, env: &mut ExecEnv<'_>) {
        match statement {
            Statement::Command(cmd) => self.run_command(cmd, env),
            Statement::Pipeline(commands) => self.run_pipeline(commands, env),
            Statement::If {
                condition,
                body,
                else_if,
                else_body,
            } => {
                if self.evaluate_if_condition(condition, env) {
                    self.execute_statements(body, env);
                    return;
                }
                for (else_if_condition, else_if_body) in else_if {
                    if self.evaluate_if_condition(else_if_condition, env) {
                        self.execute_statements(else_if_body, env);
                        return;
                    }
                }
                if let Some(else_statements) = else_body {
                    self.execute_statements(else_statements, env);
                } else {
                    // No branch ran, which counts as success.
                    *env.last_exit_code = 0;
                }
            }
            Statement::While { condition, body } => {
                // A loop whose body never runs succeeds; otherwise its status
                // is that of the last command the body ran.
                *env.last_exit_code = 0;
                while self.exit_status.is_none() && self.evaluate_if_condition(condition, env) {
                    self.execute_statements(body, env);
                }
            }
            Statement::For {
                variable,
                values,
                body,
            } => {
                *env.last_exit_code = 0;
                for val in values {
                    // Expand variables (e.g. $VAR) and globs (e.g. *.txt)
                    for final_val in expand_word(val, &self.context(env)) {
                        if self.exit_status.is_some() {
                            return;
                        }
                        self.set_var(variable, final_val);
                        self.execute_statements(body, env);
                    }
                }
            }
        }
    }

    fn evaluate_if_condition(&self, condition: &str, env: &ExecEnv<'_>) -> bool {
        let val = expand_string(condition, &self.context(env));
        !val.is_empty() && val != "0" && val != "false"
    }

    // ==========================================
    // SINGLE COMMAND
    // ==========================================

    /// The one place a command is routed. The top-level loop and every nested
    /// statement come through here, so there is a single definition of what
    /// each command name means.
    fn run_command(&mut self, cmd: &Command, env: &mut ExecEnv<'_>) {
        let cmd = &expand_command(cmd, &self.context(env));
        if cmd.program.is_empty() {
            // A line of bare assignments sets shell variables. Each value
            // can use the ones before it: `A=1 B=$A`.
            for assignment in &cmd.assignments {
                let value = expand_string(&assignment.value, &self.context(env));
                self.set_var(&assignment.name, value);
            }
            // (With no assignments, the whole command expanded to nothing,
            // as with an unset `$CMD`.)
            *env.last_exit_code = 0;
            return;
        }

        // `FOO=1 cmd`: the assignments reach this one command through its
        // environment and are undone once it has run.
        let _prefix = self.prefix_env(&cmd.assignments, env);
        self.dispatch(cmd, env);
    }

    /// Sets the environment for `FOO=1 cmd`; see [`PrefixEnv`].
    fn prefix_env(&self, assignments: &[Assignment], env: &ExecEnv<'_>) -> PrefixEnv {
        let mut saved = Vec::with_capacity(assignments.len());
        for assignment in assignments {
            let value = expand_string(&assignment.value, &self.context(env));
            saved.push((assignment.name.clone(), std::env::var_os(&assignment.name)));
            std::env::set_var(&assignment.name, value);
        }
        PrefixEnv { saved }
    }

    /// Runs an expanded command: an executor builtin, a registry builtin,
    /// or an external program, in that order.
    fn dispatch(&mut self, cmd: &Command, env: &mut ExecEnv<'_>) {
        if !self.check_permission(&cmd.program, &cmd.args, env) {
            return;
        }

        if let Some(kind) = ExecutorBuiltin::lookup(&cmd.program) {
            *env.last_exit_code = self.run_executor_builtin(kind, &cmd.args, env);
            return;
        }

        if let Some(builtin) = oxide_builtins::registry().get(&cmd.program) {
            let (code, _) = run_builtin_stage(
                builtin,
                &cmd.args,
                Upstream::None,
                cmd,
                true,
                &mut self.vars,
                env,
            );
            *env.last_exit_code = code;
            return;
        }

        self.spawn_external(cmd, &cmd.args, env);
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

            ExecutorBuiltin::Exit => {
                let status = match args {
                    [] => *env.last_exit_code,
                    // As in bash, the status wraps at 256: `exit -1` is 255.
                    [code] => match code.parse::<i64>() {
                        Ok(n) => (n & 0xff) as i32,
                        Err(_) => {
                            eprintln!("oxide: exit: {}: numeric argument required", code);
                            2
                        }
                    },
                    _ => {
                        eprintln!("oxide: exit: too many arguments");
                        return 1;
                    }
                };
                self.exit_status = Some(status);
                status
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
            let cmd = &expand_command(cmd, &self.context(env));
            if cmd.program.is_empty() {
                // A stage that expanded to nothing produces no output.
                upstream = Upstream::None;
                if is_last {
                    *env.last_exit_code = 0;
                    status_from_child = false;
                }
                continue;
            }

            // A prefix assignment applies to its own stage only.
            let _prefix = self.prefix_env(&cmd.assignments, env);

            if !self.check_permission(&cmd.program, &cmd.args, env) {
                break;
            }

            if ExecutorBuiltin::lookup(&cmd.program).is_some() {
                eprintln!("oxide: '{}' cannot be used in a pipeline yet", cmd.program);
                *env.last_exit_code = 1;
                break;
            }

            if let Some(builtin) = oxide_builtins::registry().get(&cmd.program) {
                let (code, next) = run_builtin_stage(
                    builtin,
                    &cmd.args,
                    upstream.take(),
                    cmd,
                    is_last,
                    &mut self.vars,
                    env,
                );
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
                &cmd.args,
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
                Ok(status) => crate::process::status_code(status),
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

/// Environment variables set for a single command by `FOO=1 cmd`. Dropping
/// it puts each one back as it was, so the assignment never outlives the
/// command, whether that is a builtin or a child process (which takes its
/// copy of the environment when it is spawned).
struct PrefixEnv {
    saved: Vec<(String, Option<OsString>)>,
}

impl Drop for PrefixEnv {
    fn drop(&mut self) {
        for (name, old) in self.saved.drain(..).rev() {
            match old {
                Some(value) => std::env::set_var(&name, value),
                None => std::env::remove_var(&name),
            }
        }
    }
}

/// Expands a command as typed into the command that actually runs: the
/// program and its arguments go through full word expansion, redirection
/// targets through single-string expansion.
///
/// A handful of builtins take declarations rather than ordinary arguments:
/// `alias` and `export` take `NAME=VALUE` pairs, `unset` takes names, and
/// `source` takes one path. Their arguments are expanded as single strings,
/// so `export GREETING="hello world"` or a value holding `*` arrives whole.
fn expand_command(cmd: &Command, ctx: &dyn Context) -> Command {
    let mut words = expand_word(&cmd.program, ctx).into_iter();
    let program = words.next().unwrap_or_default();
    let mut args: Vec<String> = words.collect();

    if matches!(program.as_str(), "source" | "alias" | "unset" | "export") {
        args.extend(cmd.args.iter().map(|arg| expand_string(arg, ctx)));
    } else {
        for arg in &cmd.args {
            args.extend(expand_word(arg, ctx));
        }
    }

    Command {
        program,
        args,
        assignments: cmd.assignments.clone(),
        outfile: cmd.outfile.as_deref().map(|f| expand_string(f, ctx)),
        infile: cmd.infile.as_deref().map(|f| expand_string(f, ctx)),
        append: cmd.append,
    }
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
    vars: &mut HashMap<String, String>,
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
        vars,
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

    fn ctx() -> ShellContext<'static> {
        static NO_VARS: std::sync::OnceLock<HashMap<String, String>> = std::sync::OnceLock::new();
        ShellContext {
            last_status: 0,
            vars: NO_VARS.get_or_init(HashMap::new),
        }
    }

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
    fn declaration_arguments_are_not_split_or_globbed() {
        let mut cmd = Command::new("export".to_string());
        cmd.args = vec!["GREETING=\"hello *\"".to_string(), "STAR=*".to_string()];
        assert_eq!(
            expand_command(&cmd, &ctx()).args,
            ["GREETING=hello *", "STAR=*"]
        );
    }

    #[test]
    fn quoted_program_names_and_redirects_are_unquoted() {
        let mut cmd = Command::new("'echo'".to_string());
        cmd.args = vec!["\"a  b\"".to_string()];
        cmd.outfile = Some("'out file.txt'".to_string());
        let cmd = expand_command(&cmd, &ctx());
        assert_eq!(cmd.program, "echo");
        assert_eq!(cmd.args, ["a  b"]);
        assert_eq!(cmd.outfile.as_deref(), Some("out file.txt"));
    }

    // ------------------------------------------
    // Assignments and special variables
    // ------------------------------------------

    /// Runs each line through one executor, as the REPL would, and returns
    /// the executor along with the final exit status.
    fn run(lines: &[&str]) -> (Executor, i32) {
        let mut executor = Executor::new();
        let mut mode = CompatMode::Oxide;
        let mut aliases = HashMap::new();
        let mut status = 0;
        let mut jobs = crate::jobs::JobManager::new();
        let mut cache = CommandCache::new();
        for line in lines {
            executor.execute_line(
                line,
                &mut mode,
                &mut aliases,
                &mut status,
                &mut jobs,
                &[],
                &mut cache,
            );
        }
        (executor, status)
    }

    fn var<'e>(executor: &'e Executor, name: &str) -> Option<&'e str> {
        executor.vars.get(name).map(String::as_str)
    }

    #[test]
    fn an_assignment_sets_a_shell_variable_not_an_environment_one() {
        let (executor, status) = run(&["OXIDE_EXEC_SHELL_ONLY=1"]);
        assert_eq!(status, 0);
        assert_eq!(var(&executor, "OXIDE_EXEC_SHELL_ONLY"), Some("1"));
        assert!(std::env::var_os("OXIDE_EXEC_SHELL_ONLY").is_none());
    }

    #[test]
    fn values_are_expanded_but_not_split() {
        let (executor, _) = run(&[
            "A=1 B=$A-2",
            "SPACED=\"a  b\"",
            "LITERAL='$HOME'",
            "JOINED=$SPACED$A",
        ]);
        assert_eq!(var(&executor, "B"), Some("1-2"));
        assert_eq!(var(&executor, "SPACED"), Some("a  b"));
        assert_eq!(var(&executor, "LITERAL"), Some("$HOME"));
        assert_eq!(var(&executor, "JOINED"), Some("a  b1"));
    }

    #[test]
    fn assigning_an_exported_variable_updates_the_environment() {
        let (executor, _) = run(&["export OXIDE_EXEC_EXPORTED=1", "OXIDE_EXEC_EXPORTED=2"]);
        assert_eq!(std::env::var("OXIDE_EXEC_EXPORTED").unwrap(), "2");
        assert_eq!(var(&executor, "OXIDE_EXEC_EXPORTED"), None);
    }

    #[test]
    fn dollar_question_captures_the_previous_status() {
        let (executor, status) = run(&["cd ./no/such/dir", "FAILED=$?", "PASSED=$?"]);
        assert_eq!(var(&executor, "FAILED"), Some("1"));
        assert_eq!(var(&executor, "PASSED"), Some("0"));
        // An assignment itself succeeds.
        assert_eq!(status, 0);
    }

    #[test]
    fn dollar_dollar_is_this_process() {
        let (executor, _) = run(&["PID=$$ BRACED=${$}"]);
        let pid = std::process::id().to_string();
        assert_eq!(var(&executor, "PID"), Some(pid.as_str()));
        assert_eq!(var(&executor, "BRACED"), Some(pid.as_str()));
    }

    #[test]
    fn a_prefix_assignment_does_not_outlive_its_command() {
        let (executor, status) = run(&["OXIDE_EXEC_PREFIX=1 cd ."]);
        assert_eq!(status, 0);
        assert!(std::env::var_os("OXIDE_EXEC_PREFIX").is_none());
        assert_eq!(var(&executor, "OXIDE_EXEC_PREFIX"), None);
    }

    #[test]
    fn a_for_loop_variable_is_a_shell_variable() {
        let (executor, _) = run(&["for OXIDE_EXEC_LOOP in a b c { LAST=$OXIDE_EXEC_LOOP }"]);
        assert_eq!(var(&executor, "LAST"), Some("c"));
        assert!(std::env::var_os("OXIDE_EXEC_LOOP").is_none());
    }
}
