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
                    rl.add_history_entry(trimmed)?;
                    self.state.history.push(trimmed.to_string());
                    history::append(trimmed);
                    self.execute_line(trimmed);
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
        self.run_source(&source, true);
        Ok(())
    }

    /// Runs commands piped in on stdin (`echo pwd | oxide`). Treated like a
    /// script: no prompt is shown and nothing goes into the history file.
    pub fn run_stdin(&mut self) -> anyhow::Result<()> {
        // Read everything up front so no lock on stdin is held while the
        // commands run; a builtin reading stdin would otherwise deadlock.
        let mut source = String::new();
        std::io::stdin().read_to_string(&mut source)?;
        self.run_source(&source, false);
        Ok(())
    }

    /// Runs `source` one line at a time, skipping blanks and comments.
    fn run_source(&mut self, source: &str, record_history: bool) {
        for line in source.lines() {
            let trimmed = line.trim();

            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            if record_history {
                self.state.history.push(trimmed.to_string());
                history::append(trimmed);
            }
            self.execute_line(trimmed);
            if !self.state.is_running {
                break;
            }
        }
    }

    // ==========================================
    // MODE 3: ONE COMMAND (`oxide -c`)
    // ==========================================
    /// Runs a single command string and returns its exit status. Nothing is
    /// added to the history file.
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
