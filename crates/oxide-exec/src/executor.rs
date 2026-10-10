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
use std::io::{Read, Write};
use std::rc::Rc;

use oxide_builtins::{Ctx, Io};
use oxide_compat::CompatMode;
use oxide_parser::ast::{Assignment, Command, Condition, Executable, HereInput, List, Statement};
use oxide_parser::expand::{expand_heredoc, expand_pattern, expand_string, expand_word, Context};
use oxide_parser::lexer::Lexer;
use oxide_parser::parser::Parser;
use oxide_perf::cache::CommandCache;
use oxide_security::permissions::PermissionManager;

use crate::pipeline::{builtin_sink, spawn_stage, SharedBuf, Upstream};

/// Control flow that unwinds out of whatever is running until something
/// catches it: `return` leaves the current function, `break N` the N
/// innermost loops, and `continue N` ends the current pass of the Nth.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Unwind {
    Return,
    Break(usize),
    Continue(usize),
}

/// How deep functions may call each other before the shell gives up, so a
/// function that calls itself forever fails instead of crashing the shell.
const MAX_CALL_DEPTH: usize = 200;

pub struct Executor {
    pub runtime: oxide_script::runtime::Runtime,
    pub script_scope: oxide_script::scope::Scope,
    /// Set by `exit`: the status the shell should exit with. Once it is set
    /// nothing else runs, however deeply nested the `exit` was.
    pub exit_status: Option<i32>,
    /// Shell variables (`NAME=value`). Exported ones live in the process
    /// environment instead, where child processes can see them.
    pub vars: HashMap<String, String>,
    /// The status of the last `$(...)` that ran, for an assignment that
    /// holds one: `X=$(cmd)` has the status of `cmd`.
    substitution_status: Option<i32>,
    /// Functions defined so far, by name. Shared, so a function can be
    /// redefined while it is running without pulling its body out from
    /// under it.
    pub functions: HashMap<String, Rc<List>>,
    /// How many function calls are running, one inside the next.
    call_depth: usize,
    /// `$1`, `$2`, ...: the arguments of the running function, or of the
    /// script at the top level.
    pub positional: Vec<String>,
    /// `$0`: the shell's name, or the script it is running.
    pub shell_name: String,
    /// One frame per running function call, holding what its `local`
    /// declarations hid.
    frames: Vec<oxide_script::scope::Frame>,
    /// Set by `return`, `break` or `continue` until whatever it is leaving
    /// has been left.
    unwind: Option<Unwind>,
    /// How many loops are running around the current statement, within the
    /// current function call.
    loop_depth: usize,
    /// Set when an expansion fails (say `$((1/0))`), so the command it
    /// belongs to fails instead of running.
    expansion_failed: bool,
    /// `set -e`: a command that fails, untested, ends the shell.
    errexit: bool,
    /// How many `if`/`while` conditions are running around the current
    /// command. A failure there is a test result, not an error.
    condition_depth: usize,
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
    /// While a `$(...)` runs, where its output goes instead of the terminal.
    pub capture: Option<SharedBuf>,
}

/// Builtins that can't live in `oxide-builtins` because they need the executor
/// itself: the job table, the compat mode, the script runtime, or recursion
/// back through the lexer and parser.
///
/// `oxide_builtins::EXECUTOR_BUILTINS` advertises these names to the line
/// editor; the test at the bottom of this file keeps the two in agreement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExecutorBuiltin {
    Break,
    Call,
    Continue,
    Exit,
    Import,
    Jail,
    Jobs,
    Local,
    Mode,
    Refresh,
    Return,
    Set,
    Source,
}

impl ExecutorBuiltin {
    fn lookup(name: &str) -> Option<Self> {
        Some(match name {
            "break" => Self::Break,
            "call" => Self::Call,
            "continue" => Self::Continue,
            "exit" => Self::Exit,
            "import" => Self::Import,
            "jail" => Self::Jail,
            "jobs" => Self::Jobs,
            "local" => Self::Local,
            "mode" => Self::Mode,
            "refresh" => Self::Refresh,
            "return" => Self::Return,
            "set" => Self::Set,
            "source" => Self::Source,
            _ => return None,
        })
    }
}

/// What expansion reads — the shell's own state, then the environment —
/// and the executor it runs `$(...)` substitutions in.
struct ShellContext<'a, 'e> {
    executor: &'a mut Executor,
    env: &'a mut ExecEnv<'e>,
}

impl Context for ShellContext<'_, '_> {
    fn lookup(&self, name: &str) -> Option<String> {
        let positional = &self.executor.positional;
        match name {
            "?" => Some((*self.env.last_exit_code).to_string()),
            "#" => Some(positional.len().to_string()),
            "@" | "*" => Some(positional.join(" ")),
            "0" => Some(self.executor.shell_name.clone()),
            _ if name.bytes().all(|b| b.is_ascii_digit()) => {
                let index = name.parse::<usize>().ok()?.checked_sub(1)?;
                positional.get(index).cloned()
            }
            _ => self
                .executor
                .vars
                .get(name)
                .cloned()
                .or_else(|| std::env::var(name).ok()),
        }
    }

    fn substitute(&mut self, command: &str) -> String {
        self.executor.capture_output(command, self.env)
    }

    fn positional(&self) -> Vec<String> {
        self.executor.positional.clone()
    }

    fn assign(&mut self, name: &str, value: &str) {
        self.executor.set_var(name, value.to_string());
    }

    fn report(&mut self, message: &str) {
        eprintln!("oxide: {}", message);
        self.executor.expansion_failed = true;
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
            substitution_status: None,
            functions: HashMap::new(),
            call_depth: 0,
            positional: Vec::new(),
            shell_name: "oxide".to_string(),
            frames: Vec::new(),
            unwind: None,
            loop_depth: 0,
            expansion_failed: false,
            errexit: false,
            condition_depth: 0,
        }
    }

    /// The expansion context for the current state.
    fn context<'a, 'e>(&'a mut self, env: &'a mut ExecEnv<'e>) -> ShellContext<'a, 'e> {
        ShellContext {
            executor: self,
            env,
        }
    }

    /// Runs `command` as a `$(...)` substitution and returns what it wrote
    /// to stdout.
    ///
    /// Like a subshell, it can't change the shell around it: variables, the
    /// working directory and an `exit` inside it stay inside it.
    fn capture_output(&mut self, command: &str, env: &mut ExecEnv<'_>) -> String {
        let buf = SharedBuf::default();
        let outer_capture = env.capture.replace(buf.clone());
        let vars = self.vars.clone();
        let cwd = std::env::current_dir().ok();
        let exit_status = self.exit_status.take();

        self.run_input(command, env);

        self.substitution_status = Some(*env.last_exit_code);
        self.exit_status = exit_status;
        if let Some(cwd) = cwd {
            let _ = std::env::set_current_dir(cwd);
        }
        self.vars = vars;
        env.capture = outer_capture;
        String::from_utf8_lossy(&buf.take()).into_owned()
    }

    /// Sets a variable: in the environment if it is exported there,
    /// otherwise as a shell variable.
    fn set_var(&mut self, name: &str, value: String) {
        // A shell variable (a local, say) wins over an exported one of the
        // same name; otherwise exported names stay exported.
        if !self.vars.contains_key(name) && std::env::var_os(name).is_some() {
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
            capture: None,
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

        self.execute_list(&executables, env);

        exec_timer.stop("Execution Engine");
    }

    /// Whether running should stop here: the shell is exiting, or a
    /// `return`, `break` or `continue` is on its way out.
    fn stopped(&self) -> bool {
        self.exit_status.is_some() || self.unwind.is_some()
    }

    /// Runs a list of statements, honoring `&&` and `||` between them.
    fn execute_list(&mut self, list: &[Executable], env: &mut ExecEnv<'_>) {
        for (i, exec) in list.iter().enumerate() {
            if self.stopped() {
                return;
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

            // `set -e`: a failure ends the shell, unless its status is being
            // tested, by a condition or by a `&&`/`||` that follows it.
            if self.errexit
                && *env.last_exit_code != 0
                && self.condition_depth == 0
                && !self.stopped()
            {
                let tested = matches!(
                    list.get(i + 1).map(|next| &next.condition),
                    Some(Condition::And | Condition::Or)
                );
                if !tested {
                    self.exit_status = Some(*env.last_exit_code);
                }
            }
        }
    }

    /// After one pass of a loop body: catches a `break` or `continue` aimed
    /// at this loop, and says whether the loop should stop.
    fn end_of_pass(&mut self) -> bool {
        match self.unwind {
            Some(Unwind::Break(n)) => {
                // `break 2` stops this loop and leaves a `break 1` for the
                // one around it.
                self.unwind = (n > 1).then(|| Unwind::Break(n - 1));
                true
            }
            Some(Unwind::Continue(n)) if n > 1 => {
                self.unwind = Some(Unwind::Continue(n - 1));
                true
            }
            Some(Unwind::Continue(_)) => {
                self.unwind = None;
                false
            }
            Some(Unwind::Return) => true,
            None => self.exit_status.is_some(),
        }
    }

    /// Runs a condition list and says whether it succeeded: whether its
    /// last command exited with status 0.
    fn condition_holds(&mut self, condition: &[Executable], env: &mut ExecEnv<'_>) -> bool {
        self.condition_depth += 1;
        self.execute_list(condition, env);
        self.condition_depth -= 1;
        !self.stopped() && *env.last_exit_code == 0
    }

    fn execute_statement(&mut self, statement: &Statement, env: &mut ExecEnv<'_>) {
        match statement {
            Statement::Command(cmd) => self.run_command(cmd, env),
            Statement::Pipeline(commands) => self.run_pipeline(commands, env),
            Statement::Case { word, arms } => {
                let word = expand_string(word, &mut self.context(env));
                for arm in arms {
                    for pattern in &arm.patterns {
                        let pattern = expand_pattern(pattern, &mut self.context(env));
                        if pattern_matches(&pattern, &word) {
                            self.execute_list(&arm.body, env);
                            return;
                        }
                    }
                }
                // No arm matched, which counts as success.
                *env.last_exit_code = 0;
            }
            Statement::Function { name, body } => {
                self.functions.insert(name.clone(), Rc::new(body.clone()));
                *env.last_exit_code = 0;
            }
            Statement::If {
                condition,
                body,
                else_if,
                else_body,
            } => {
                if self.condition_holds(condition, env) {
                    self.execute_list(body, env);
                    return;
                }
                for (else_if_condition, else_if_body) in else_if {
                    if self.condition_holds(else_if_condition, env) {
                        self.execute_list(else_if_body, env);
                        return;
                    }
                }
                if let Some(else_statements) = else_body {
                    self.execute_list(else_statements, env);
                } else if !self.stopped() {
                    // No branch ran, which counts as success.
                    *env.last_exit_code = 0;
                }
            }
            Statement::While { condition, body } => {
                // A loop whose body never runs succeeds; otherwise its status
                // is that of the last command the body ran, not the failed
                // condition that ended it.
                let mut status = 0;
                self.loop_depth += 1;
                while self.condition_holds(condition, env) {
                    self.execute_list(body, env);
                    status = *env.last_exit_code;
                    if self.end_of_pass() {
                        break;
                    }
                }
                self.loop_depth -= 1;
                if self.exit_status.is_none() {
                    *env.last_exit_code = status;
                }
            }
            Statement::For {
                variable,
                values,
                body,
            } => {
                *env.last_exit_code = 0;
                // Every value is expanded (variables, globs like *.txt) before
                // the first pass, as in other shells.
                let mut items = Vec::new();
                for val in values {
                    items.extend(expand_word(val, &mut self.context(env)));
                }
                self.loop_depth += 1;
                for item in items {
                    self.set_var(variable, item);
                    self.execute_list(body, env);
                    if self.end_of_pass() {
                        break;
                    }
                }
                self.loop_depth -= 1;
            }
        }
    }

    // ==========================================
    // SINGLE COMMAND
    // ==========================================

    /// The one place a command is routed. The top-level loop and every nested
    /// statement come through here, so there is a single definition of what
    /// each command name means.
    fn run_command(&mut self, cmd: &Command, env: &mut ExecEnv<'_>) {
        self.expansion_failed = false;
        let cmd = &expand_command(cmd, &mut self.context(env));
        if std::mem::take(&mut self.expansion_failed) {
            *env.last_exit_code = 1;
            return;
        }
        if cmd.program.is_empty() {
            // A line of bare assignments sets shell variables. Each value
            // can use the ones before it: `A=1 B=$A`.
            self.substitution_status = None;
            for assignment in &cmd.assignments {
                let value = expand_string(&assignment.value, &mut self.context(env));
                if std::mem::take(&mut self.expansion_failed) {
                    *env.last_exit_code = 1;
                    return;
                }
                self.set_var(&assignment.name, value);
            }
            // It succeeds, unless a `$(...)` in it ran: then it has that
            // command's status. (With no assignments at all, the whole
            // command expanded to nothing, as with an unset `$CMD`.)
            *env.last_exit_code = self.substitution_status.take().unwrap_or(0);
            return;
        }

        // `FOO=1 cmd`: the assignments reach this one command through its
        // environment and are undone once it has run.
        let _prefix = self.prefix_env(&cmd.assignments, env);
        self.dispatch(cmd, env);
    }

    /// Sets the environment for `FOO=1 cmd`; see [`PrefixEnv`].
    fn prefix_env(&mut self, assignments: &[Assignment], env: &mut ExecEnv<'_>) -> PrefixEnv {
        let mut saved = Vec::with_capacity(assignments.len());
        for assignment in assignments {
            let value = expand_string(&assignment.value, &mut self.context(env));
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

        // A function shadows any builtin or program of the same name.
        if let Some(body) = self.functions.get(&cmd.program).cloned() {
            *env.last_exit_code = if cmd.outfile.is_some() || cmd.out_to_err {
                // Redirected: gather the body's output, then send it on.
                let (code, output) = self.call_function_captured(cmd, &body, env);
                send_output(cmd, &output, env);
                code
            } else {
                self.call_function(&cmd.program, &cmd.args, &body, env)
            };
            return;
        }

        if let Some(kind) = ExecutorBuiltin::lookup(&cmd.program) {
            *env.last_exit_code = self.run_executor_builtin(kind, &cmd.args, env);
            return;
        }

        if let Some(builtin) = oxide_builtins::registry().get(&cmd.program) {
            let input = crate::process::here_bytes(cmd)
                .map(Upstream::Buffer)
                .unwrap_or(Upstream::None);
            let (code, _) = self.run_builtin_stage(builtin, &cmd.args, input, cmd, true, env);
            *env.last_exit_code = code;
            return;
        }

        self.spawn_external(cmd, &cmd.args, env);
    }

    /// Runs a registry builtin, wiring its three streams from `upstream` and the
    /// command's redirects. Returns the exit code and what the next pipeline
    /// stage should read.
    fn run_builtin_stage(
        &mut self,
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

        let mut sink = match builtin_sink(is_last, &cmd.outfile, cmd.append, env.capture.as_ref()) {
            Ok(sink) => sink,
            Err(e) => {
                eprintln!("{}", e);
                return (1, Upstream::None);
            }
        };

        let mut stderr: Box<dyn Write> = match &cmd.errfile {
            Some(path) => match crate::redirect::open_output(path, cmd.err_append) {
                Ok(file) => Box::new(file),
                Err(e) => {
                    eprintln!("oxide: redirection error: {}: {}", path, e);
                    return (1, Upstream::None);
                }
            },
            None => Box::new(std::io::stderr()),
        };
        let functions = &self.functions;
        let is_function = |name: &str| functions.contains_key(name);
        let mut ctx = Ctx {
            aliases: &mut *env.aliases,
            history: env.history,
            vars: &mut self.vars,
            is_function: &is_function,
        };

        let code = if cmd.err_to_out || cmd.out_to_err {
            // `2>&1` / `>&2`: both streams write into one buffer, so lines keep
            // the order they were written in, and it all goes where they were
            // joined.
            let merged = SharedBuf::default();
            let (mut out, mut err) = (merged.clone(), merged.clone());
            let code = {
                let mut io = Io::new(&mut *reader, &mut out, &mut err);
                builtin.run(args, &mut io, &mut ctx)
            };
            let dest: &mut dyn Write = if cmd.err_to_out {
                sink.as_writer()
            } else {
                &mut *stderr
            };
            let _ = dest.write_all(&merged.take());
            code
        } else {
            let mut io = Io::new(&mut *reader, sink.as_writer(), &mut *stderr);
            builtin.run(args, &mut io, &mut ctx)
        };

        // Drop the read end before the caller waits on any upstream process. A
        // builtin that ignores its input (`ls | echo hi`) would otherwise leave
        // the producer blocked on a pipe nobody is draining.
        drop(reader);

        (code, sink.finish())
    }

    /// Runs a function's body in this shell, with `args` as its `$1`, `$2`,
    /// ..., and returns its status.
    fn call_function(
        &mut self,
        name: &str,
        args: &[String],
        body: &List,
        env: &mut ExecEnv<'_>,
    ) -> i32 {
        if self.call_depth >= MAX_CALL_DEPTH {
            eprintln!(
                "oxide: {}: maximum function nesting level ({}) exceeded",
                name, MAX_CALL_DEPTH
            );
            return 1;
        }
        let caller_args = std::mem::replace(&mut self.positional, args.to_vec());
        // A function can't break out of a loop it was called from.
        let caller_loops = std::mem::replace(&mut self.loop_depth, 0);
        self.call_depth += 1;
        self.frames.push(oxide_script::scope::Frame::default());
        self.execute_list(body, env);
        // A `return` stops here, at the function it was leaving.
        self.unwind = None;
        if let Some(frame) = self.frames.pop() {
            frame.restore(&mut self.vars);
        }
        self.call_depth -= 1;
        self.loop_depth = caller_loops;
        self.positional = caller_args;
        *env.last_exit_code
    }

    /// Calls a function with everything it writes to stdout gathered up, for
    /// a call that is redirected or feeds a pipeline. Unlike `$(...)` it
    /// runs in this shell, so its variables stick.
    fn call_function_captured(
        &mut self,
        cmd: &Command,
        body: &List,
        env: &mut ExecEnv<'_>,
    ) -> (i32, Vec<u8>) {
        let buf = SharedBuf::default();
        let outer = env.capture.replace(buf.clone());
        let code = self.call_function(&cmd.program, &cmd.args, body, env);
        env.capture = outer;
        (code, buf.take())
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
            match crate::process::spawn_background(&program_path, &args, cmd) {
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
            *env.last_exit_code =
                crate::process::spawn_single(&program_path, &args, cmd, env.capture.as_ref());
        }
    }

    fn run_executor_builtin(
        &mut self,
        kind: ExecutorBuiltin,
        args: &[String],
        env: &mut ExecEnv<'_>,
    ) -> i32 {
        match kind {
            ExecutorBuiltin::Local => {
                let Some(frame) = self.frames.last_mut() else {
                    eprintln!("oxide: local: can only be used in a function");
                    return 1;
                };
                let mut status = 0;
                for arg in args {
                    let (name, value) = match arg.split_once('=') {
                        Some((name, value)) => (name, Some(value)),
                        None => (arg.as_str(), None),
                    };
                    if oxide_parser::ast::Assignment::parse(&format!("{}=", name)).is_none() {
                        eprintln!("oxide: local: '{}': not a valid identifier", name);
                        status = 1;
                        continue;
                    }
                    frame.save(name, self.vars.get(name).cloned());
                    // `local NAME` alone hides the outer value until set.
                    self.vars
                        .insert(name.to_string(), value.unwrap_or_default().to_string());
                }
                status
            }

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

            ExecutorBuiltin::Break | ExecutorBuiltin::Continue => {
                let name = if kind == ExecutorBuiltin::Break {
                    "break"
                } else {
                    "continue"
                };
                if self.loop_depth == 0 {
                    eprintln!("oxide: {}: only meaningful in a loop", name);
                    return 1;
                }
                let count = match args {
                    [] => 1,
                    [n] => match n.parse::<usize>() {
                        Ok(n) if n >= 1 => n,
                        _ => {
                            eprintln!("oxide: {}: {}: loop count out of range", name, n);
                            return 1;
                        }
                    },
                    _ => {
                        eprintln!("oxide: {}: too many arguments", name);
                        return 1;
                    }
                };
                // Asking to leave more loops than there are leaves them all.
                let count = count.min(self.loop_depth);
                self.unwind = Some(if kind == ExecutorBuiltin::Break {
                    Unwind::Break(count)
                } else {
                    Unwind::Continue(count)
                });
                0
            }

            ExecutorBuiltin::Return => {
                if self.call_depth == 0 {
                    eprintln!("oxide: return: can only return from a function");
                    return 1;
                }
                let status = match args {
                    [] => *env.last_exit_code,
                    [code] => match code.parse::<i64>() {
                        Ok(n) => (n & 0xff) as i32,
                        Err(_) => {
                            eprintln!("oxide: return: {}: numeric argument required", code);
                            2
                        }
                    },
                    _ => {
                        eprintln!("oxide: return: too many arguments");
                        return 1;
                    }
                };
                self.unwind = Some(Unwind::Return);
                status
            }

            ExecutorBuiltin::Set => {
                if args.is_empty() {
                    // With no options, list the shell variables.
                    let mut vars: Vec<_> = self.vars.iter().collect();
                    vars.sort();
                    for (name, value) in vars {
                        println!("{}={}", name, value);
                    }
                    return 0;
                }
                for arg in args {
                    // `-e` turns an option on, `+e` turns it off.
                    let (on, flags) = match (arg.strip_prefix('-'), arg.strip_prefix('+')) {
                        (Some(flags), _) | (_, Some(flags)) if !flags.is_empty() => {
                            (arg.starts_with('-'), flags)
                        }
                        _ => {
                            eprintln!("oxide: set: {}: invalid option", arg);
                            return 2;
                        }
                    };
                    for flag in flags.chars() {
                        match flag {
                            'e' => self.errexit = on,
                            _ => {
                                eprintln!("oxide: set: -{}: invalid option", flag);
                                return 2;
                            }
                        }
                    }
                }
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
                        let list = Parser::new(tokens).parse();
                        // A sourced file runs in *this* shell, so it goes
                        // through the executor, not the script runtime.
                        self.execute_list(&list, env);
                        *env.last_exit_code
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
            let cmd = &expand_command(cmd, &mut self.context(env));
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

            // A here-document replaces whatever the previous stage sent.
            if let Some(bytes) = crate::process::here_bytes(cmd) {
                upstream = Upstream::Buffer(bytes);
            }

            if !self.check_permission(&cmd.program, &cmd.args, env) {
                break;
            }

            if let Some(body) = self.functions.get(&cmd.program).cloned() {
                // A function's output is gathered and handed on like a
                // builtin's.
                let (code, output) = self.call_function_captured(cmd, &body, env);
                if is_last {
                    send_output(cmd, &output, env);
                    *env.last_exit_code = code;
                    status_from_child = false;
                } else {
                    upstream = Upstream::Buffer(output);
                }
                continue;
            }

            if ExecutorBuiltin::lookup(&cmd.program).is_some() {
                eprintln!("oxide: '{}' cannot be used in a pipeline yet", cmd.program);
                *env.last_exit_code = 1;
                break;
            }

            if let Some(builtin) = oxide_builtins::registry().get(&cmd.program) {
                let (code, next) =
                    self.run_builtin_stage(builtin, &cmd.args, upstream.take(), cmd, is_last, env);
                upstream = next;
                if is_last {
                    *env.last_exit_code = code;
                    status_from_child = false;
                }
                continue;
            }

            let program_path = resolve_program_path(&cmd.program, env.command_cache);
            // The last stage pipes its output only to be captured by `$(...)`.
            let pipe_output = !is_last || env.capture.is_some();
            match spawn_stage(&program_path, &cmd.args, upstream.take(), pipe_output, cmd) {
                Ok(mut stage) => {
                    if is_last {
                        status_from_child = true;
                    } else {
                        upstream = stage
                            .output
                            .take()
                            .map(Upstream::Pipe)
                            .unwrap_or(Upstream::None);
                    }
                    spawned.push(stage);
                }
                Err(message) => {
                    crate::redirect::report(&cmd.errfile, cmd.err_append, &message);
                    *env.last_exit_code = 127;
                    break;
                }
            }
        }

        // Release any dangling read end before reaping, so a producer blocked
        // on a full pipe gets a broken-pipe error instead of hanging us.
        drop(upstream);

        // Inside `$(...)`, an external last stage's output is the result.
        if let (Some(capture), Some(stage)) = (&env.capture, spawned.last_mut()) {
            if let Some(mut output) = stage.output.take() {
                let mut bytes = Vec::new();
                let _ = output.read_to_end(&mut bytes);
                capture.push(&bytes);
            }
        }

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

/// Whether `word` matches the glob `pattern`, as a `case` arm tests it: the
/// whole word, with `*` free to match `/` too. A malformed pattern only
/// matches itself.
fn pattern_matches(pattern: &str, word: &str) -> bool {
    match glob::Pattern::new(pattern) {
        Ok(compiled) => compiled.matches(word),
        Err(_) => pattern == word,
    }
}

/// Writes output gathered from a function to where `cmd` sends its stdout:
/// a redirect target, stderr for `>&2`, a `$(...)` capture, or the terminal.
fn send_output(cmd: &Command, output: &[u8], env: &ExecEnv<'_>) {
    if cmd.out_to_err {
        let _ = std::io::stderr().write_all(output);
        return;
    }
    match builtin_sink(true, &cmd.outfile, cmd.append, env.capture.as_ref()) {
        Ok(mut sink) => {
            let _ = sink.as_writer().write_all(output);
            sink.finish();
        }
        Err(e) => eprintln!("{}", e),
    }
}

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
fn expand_command(cmd: &Command, ctx: &mut dyn Context) -> Command {
    let mut words = expand_word(&cmd.program, ctx).into_iter();
    let program = words.next().unwrap_or_default();
    let mut args: Vec<String> = words.collect();

    let declaration = matches!(
        program.as_str(),
        "source" | "alias" | "unset" | "export" | "local" | "let"
    );
    for arg in &cmd.args {
        if declaration {
            args.push(expand_string(arg, ctx));
        } else {
            args.extend(expand_word(arg, ctx));
        }
    }

    let mut target = |file: &Option<String>| file.as_deref().map(|f| expand_string(f, ctx));
    let outfile = target(&cmd.outfile);
    let infile = target(&cmd.infile);
    let errfile = target(&cmd.errfile);

    // A here-document's body is expanded now, unless its delimiter was
    // quoted; either way what is left is fed to stdin as it stands.
    let here = cmd.here.as_ref().map(|here| match here {
        HereInput::Document { body, expand } => HereInput::Document {
            body: if *expand {
                expand_heredoc(body, ctx)
            } else {
                body.clone()
            },
            expand: false,
        },
        HereInput::String(word) => HereInput::String(expand_string(word, ctx)),
    });

    Command {
        program,
        args,
        assignments: cmd.assignments.clone(),
        outfile,
        infile,
        append: cmd.append,
        errfile,
        err_append: cmd.err_append,
        err_to_out: cmd.err_to_out,
        out_to_err: cmd.out_to_err,
        here,
    }
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

    /// Expansion with no shell around it: just the environment.
    struct NoShell;

    impl Context for NoShell {
        fn lookup(&self, name: &str) -> Option<String> {
            std::env::var(name).ok()
        }

        fn substitute(&mut self, _command: &str) -> String {
            String::new()
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
            expand_command(&cmd, &mut NoShell).args,
            ["GREETING=hello *", "STAR=*"]
        );
    }

    #[test]
    fn quoted_program_names_and_redirects_are_unquoted() {
        let mut cmd = Command::new("'echo'".to_string());
        cmd.args = vec!["\"a  b\"".to_string()];
        cmd.outfile = Some("'out file.txt'".to_string());
        let cmd = expand_command(&cmd, &mut NoShell);
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

    #[test]
    fn a_definition_stores_the_function() {
        let (executor, status) = run(&["greet() { echo hi }", "function twice { a; b }"]);
        assert_eq!(status, 0);
        let mut names: Vec<_> = executor.functions.keys().cloned().collect();
        names.sort();
        assert_eq!(names, ["greet", "twice"]);
        assert_eq!(executor.functions["twice"].len(), 2);
    }
}
