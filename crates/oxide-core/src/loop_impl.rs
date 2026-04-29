use crate::shell::Shell;
use std::fs::File;
use std::env;
use std::io::{BufRead, BufReader};

use rustyline::error::ReadlineError;
use rustyline::Editor; // Changed from DefaultEditor
use rustyline::history::DefaultHistory; // Needed for the new Editor type

// Import our new auto-completer!
use crate::completion::OxideHelper;
use rustyline::completion::FilenameCompleter;

impl Shell {
    // ==========================================
    // MODE 1: INTERACTIVE KEYBOARD (REPL)
    // ==========================================
    pub fn run_repl(&mut self) -> anyhow::Result<()> {
        
        // 1. Turn on the Signal Shield!
        oxide_exec::signals::init();

        // 2. Create the Editor with our custom OxideHelper attached!
        let mut rl: Editor<OxideHelper, DefaultHistory> = Editor::new()?;
        
        let helper = OxideHelper {
            completer: FilenameCompleter::new(),
        };
        rl.set_helper(Some(helper));

        let _ = rl.load_history("history.txt");

        while self.state.is_running {
            // ... [The rest of your loop stays exactly the same!]
            let cwd = env::current_dir()?;
            let cwd_str = cwd.display().to_string();

            // 1. Build a clean, plain-text prompt
            let prompt = format!("oxide {} > ", cwd_str);

            // 2. Hand it directly to rustyline. No truecolor, no flush()!
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
            &mut self.state.job_manager
        );
    }
}