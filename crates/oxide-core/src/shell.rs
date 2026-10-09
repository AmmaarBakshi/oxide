use oxide_builtins::history as history_builtin;
use oxide_compat::CompatMode;
use oxide_exec::executor::Executor;
use oxide_exec::jobs::JobManager;
use oxide_perf::cache::CommandCache;
use std::collections::HashMap; // <-- 1. Import it

pub struct ShellState {
    pub is_running: bool,
    pub last_exit_code: i32,
    /// The status passed to `exit`, once it has run.
    pub exit_status: Option<i32>,
    pub aliases: HashMap<String, String>,
    pub mode: CompatMode,
    pub job_manager: JobManager,
    pub history: Vec<String>,
    pub command_cache: CommandCache, // <-- 2. Add it here
    /// Lives as long as the session, so variables set on one line are
    /// still there on the next.
    pub executor: Executor,
}

impl Default for ShellState {
    fn default() -> Self {
        Self::new()
    }
}

impl ShellState {
    pub fn new() -> Self {
        Self {
            is_running: true,
            last_exit_code: 0,
            exit_status: None,
            aliases: HashMap::new(),
            mode: CompatMode::Oxide,
            job_manager: JobManager::new(),
            history: history_builtin::load(),
            command_cache: CommandCache::new(), // <-- 3. Initialize it
            executor: Executor::new(),
        }
    }
}

pub struct Shell {
    pub state: ShellState,
}

impl Default for Shell {
    fn default() -> Self {
        Self::new()
    }
}

impl Shell {
    pub fn new() -> Self {
        Self {
            state: ShellState::new(),
        }
    }
}
