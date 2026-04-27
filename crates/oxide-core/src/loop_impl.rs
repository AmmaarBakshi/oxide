use crate::shell::Shell;
use std::fs::File;
use std::env;
use std::io::{BufRead, BufReader};

use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;

impl Shell {
    // ==========================================
    // MODE 1: INTERACTIVE KEYBOARD (REPL)
    // ==========================================
    pub fn run_repl(&mut self) -> anyhow::Result<()> {
        println!("⚗️ Oxide Shell Core v0.1.0");
        
        let mut rl = DefaultEditor::new()?;
        let _ = rl.load_history("history.txt");

        while self.state.is_running {
            let cwd = env::current_dir()?;
            let cwd_str = cwd.display().to_string();

            let raw_prompt = env::var("PROMPT").unwrap_or_else(|_| "oxide $CWD > ".to_string());
            let prompt = raw_prompt.replace("$CWD", &cwd_str);

            match rl.readline(&prompt) {
                Ok(line) => {
                    let trimmed = line.trim();
                    if trimmed.is_empty() { continue; }
                    rl.add_history_entry(trimmed)?;
                    if trimmed == "exit" {
                        self.state.is_running = false;
                        break;
                    }
                    self.execute_line(trimmed); 
                },
                Err(ReadlineError::Interrupted) => { println!("^C"); continue; },
                Err(ReadlineError::Eof) => { self.state.is_running = false; break; },
                Err(err) => { println!("Error: {:?}", err); break; }
            }
        }
        let _ = rl.save_history("history.txt");
        Ok(())
    }

    // ==========================================
    // MODE 2: FILE AUTOMATION (SCRIPT)
    // ==========================================
    pub fn run_script(&mut self, file_path: &str) -> anyhow::Result<()> {
        let file = File::open(file_path)?;
        let reader = BufReader::new(file);

        for line in reader.lines() {
            let line = line?;
            let trimmed = line.trim();
            
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if trimmed == "exit" {
                break;
            }

            self.execute_line(trimmed);
        }
        Ok(())
    }

    // ==========================================
    // THE SHARED EXECUTION ENGINE
    // ==========================================
    pub fn execute_line(&mut self, input: &str) {
        // Clean up any background jobs that finished while we were typing
        self.state.job_manager.check_completed(); 
        
        let mut executor = oxide_exec::executor::Executor::new();
        
        executor.execute_line(
            input, 
            &mut self.state.mode, 
            &mut self.state.aliases, 
            &mut self.state.last_exit_code,
            &mut self.state.job_manager // <-- Pass it to the executor
        );
    }
}