use crate::shell::Shell;
use std::env;
use std::io::Read;

use rustyline::error::ReadlineError;

use oxide_builtins::history;

impl Shell {
    // ==========================================
    // MODE 1: INTERACTIVE KEYBOARD (REPL)
    // ==========================================
    pub fn run_repl(&mut self) -> anyhow::Result<()> {
        // 1. Turn on the Signal Shield!
        oxide_exec::signals::init();
        self.state.job_manager.interactive = true;

        // 2. Load user config (prompt + theme) and build the interactive
        //    editor (highlighting, completion, hints).
        let config = oxide_config::Config::load();
        let mut rl = oxide_ui::build_editor(&config)?;

        let history_path = history::history_path();
        let _ = rl.load_history(&history_path);

        while self.state.is_running {
            // ... [The rest of your loop stays exactly the same!]
            let cwd = env::current_dir()?;
            let cwd_str = cwd.display().to_string();

            // 1. Build a clean, plain-text prompt (coloring is applied by the
            //    editor's highlighter so cursor math stays correct).
            let prompt = oxide_ui::prompt::render(&config.prompt.template, &cwd_str);

            // 2. Hand it to the editor.
            match rl.readline(&prompt) {
                Ok(line) => {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    // Ctrl+C at a `> ` prompt drops the whole statement.
                    let Some(input) = complete_statement(trimmed, || rl.readline("> ")) else {
                        println!("^C");
                        continue;
                    };
                    rl.add_history_entry(input.as_str())?;
                    self.state.history.push(input.clone());
                    history::append(&input);

                    self.execute_line(&input);
                }
                Err(ReadlineError::Interrupted) => {
                    println!("^C");
                    continue;
                }
                Err(ReadlineError::Eof) => {
                    self.state.is_running = false;
                    break;
                }
                Err(err) => {
                    println!("Error: {:?}", err);
                    break;
                }
            }
        }
        let history_path = history::history_path();
        let _ = rl.save_history(&history_path);
        Ok(())
    }

    // ==========================================
    // MODE 2: FILE AUTOMATION (SCRIPT)
    // ==========================================
    pub fn run_script(&mut self, file_path: &str) -> anyhow::Result<()> {
        let source = std::fs::read_to_string(file_path)?;
        self.run_source(&source, Some(file_path));
        Ok(())
    }

    /// Runs commands piped in on stdin (`echo pwd | oxide`). Treated like a
    /// script: no prompt is shown and nothing goes into the history file.
    pub fn run_stdin(&mut self) -> anyhow::Result<()> {
        // Read everything up front so no lock on stdin is held while the
        // commands run; a builtin reading stdin would otherwise deadlock.
        let mut source = String::new();
        std::io::stdin().read_to_string(&mut source)?;
        self.run_source(&source, None);
        Ok(())
    }

    /// Runs `source` one complete statement at a time. A statement that
    /// spans lines — a block, a quoted string, a line ending in `|` — is
    /// gathered up before it runs. Scripts don't add to the history file.
    ///
    /// With a `file`, errors name it and the line the statement starts on.
    fn run_source(&mut self, source: &str, file: Option<&str>) {
        let mut pending = String::new();
        let mut start = 0;
        for (number, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            if pending.is_empty() {
                if trimmed.is_empty() || trimmed.starts_with('#') {
                    continue;
                }
                start = number + 1;
            }
            pending.push_str(line);
            pending.push('\n');
            if oxide_parser::is_incomplete(&pending) {
                continue;
            }
            self.run_statement(&std::mem::take(&mut pending), file, start);
            if !self.state.is_running {
                return;
            }
        }
        // Whatever is still open at the end runs as it stands.
        if !pending.is_empty() {
            self.run_statement(&pending, file, start);
        }
    }

    /// Runs one statement of a script, which starts on `line` of `file`.
    fn run_statement(&mut self, statement: &str, file: Option<&str>, line: usize) {
        if let Some(file) = file {
            oxide_exec::location::set(file, line);
        }
        self.execute_line(statement);
        oxide_exec::location::clear();
    }

    // ==========================================
    // MODE 3: ONE COMMAND (`oxide -c`)
    // ==========================================
    /// Runs a single command string and returns its exit status. Nothing is
    /// added to the history file.
    /// Sets `$0` (when `name` is given) and the positional parameters `$1`,
    /// `$2`, ... for a script or a `-c` command.
    pub fn set_arguments(&mut self, name: Option<String>, args: Vec<String>) {
        if let Some(name) = name {
            self.state.executor.shell_name = name;
        }
        self.state.executor.positional = args;
    }

    pub fn run_command(&mut self, command: &str) -> i32 {
        self.execute_line(command);
        self.state.last_exit_code
    }

    // ==========================================
    // THE SHARED EXECUTION ENGINE
    // ==========================================
    pub fn execute_line(&mut self, input: &str) {
        // Clean up any background jobs that finished while we were typing
        self.state.job_manager.check_completed();

        let state = &mut self.state;
        state.executor.execute_line(
            input,
            &mut state.mode,
            &mut state.aliases,
            &mut state.last_exit_code,
            &mut state.job_manager,
            &state.history,
            &mut state.command_cache,
        );

        if let Some(status) = state.executor.exit_status {
            self.state.exit_status = Some(status);
            self.state.is_running = false;
        }
    }
}

/// Reads further lines with `more` for as long as `input` is unfinished:
/// an open block, quote or `$(`, a here-document still missing its
/// delimiter, or a line ending in `|`, `&&`, `||` or `\`.
///
/// At end of input what was typed runs as it stands. `None` means the
/// statement was abandoned with Ctrl+C.
fn complete_statement(
    input: &str,
    mut more: impl FnMut() -> Result<String, ReadlineError>,
) -> Option<String> {
    let mut input = input.to_string();
    while oxide_parser::is_incomplete(&input) {
        match more() {
            Ok(line) => {
                input.push('\n');
                input.push_str(&line);
            }
            Err(ReadlineError::Interrupted) => return None,
            Err(_) => break,
        }
    }
    Some(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Feeds `lines` to [`complete_statement`] one at a time, then ends.
    fn complete(first: &str, lines: &[&str]) -> (Option<String>, usize) {
        let mut lines = lines.iter();
        let mut asked = 0;
        let input = complete_statement(first, || {
            asked += 1;
            lines
                .next()
                .map(|l| l.to_string())
                .ok_or(ReadlineError::Eof)
        });
        (input, asked)
    }

    #[test]
    fn a_complete_line_is_not_extended() {
        assert_eq!(complete("echo hi", &["never"]), (Some("echo hi".into()), 0));
    }

    #[test]
    fn open_blocks_quotes_and_pipes_ask_for_more() {
        assert_eq!(
            complete("if true {", &["echo yes", "}", "never"]),
            (Some("if true {\necho yes\n}".into()), 2)
        );
        assert_eq!(
            complete("echo 'a", &["b'"]),
            (Some("echo 'a\nb'".into()), 1)
        );
        assert_eq!(complete("ls |", &["sort"]), (Some("ls |\nsort".into()), 1));
    }

    #[test]
    fn here_documents_read_up_to_their_delimiter() {
        assert_eq!(
            complete("cat <<EOF", &["one", "two", "EOF", "never"]),
            (Some("cat <<EOF\none\ntwo\nEOF".into()), 3)
        );
    }

    #[test]
    fn end_of_input_runs_what_was_typed() {
        assert_eq!(
            complete("for x in a {", &[]),
            (Some("for x in a {".into()), 1)
        );
    }

    #[test]
    fn ctrl_c_abandons_the_statement() {
        let input = complete_statement("while true {", || Err(ReadlineError::Interrupted));
        assert_eq!(input, None);
    }
}
