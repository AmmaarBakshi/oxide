use std::collections::HashMap;
use oxide_compat::CompatMode; 
use oxide_exec::jobs::JobManager;
use oxide_builtins::history as history_builtin;
use oxide_perf::cache::CommandCache; // <-- 1. Import it

pub struct ShellState {
    pub is_running: bool,
    pub last_exit_code: i32,
    pub aliases: HashMap<String, String>,
    pub mode: CompatMode, 
    pub job_manager: JobManager, 
    pub history: Vec<String>,
    pub command_cache: CommandCache, // <-- 2. Add it here
}

impl ShellState {
    pub fn new() -> Self {
        Self {
            is_running: true,
            last_exit_code: 0,
            aliases: HashMap::new(),
            mode: CompatMode::Oxide, 
            job_manager: JobManager::new(),
            history: history_builtin::load(),
            command_cache: CommandCache::new(), // <-- 3. Initialize it
        }
    }
}

pub struct Shell {
    pub state: ShellState,
}

impl Shell {
    pub fn new() -> Self {
        Self { state: ShellState::new() }
    }
}