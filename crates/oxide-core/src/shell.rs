use std::collections::HashMap;
use oxide_compat::CompatMode; // <-- 1. Import it

pub struct ShellState {
    pub is_running: bool,
    pub last_exit_code: i32,
    pub aliases: HashMap<String, String>,
    pub mode: CompatMode, // <-- 2. Add the mode tracker
}

impl ShellState {
    pub fn new() -> Self {
        Self {
            is_running: true,
            last_exit_code: 0,
            aliases: HashMap::new(),
            mode: CompatMode::Oxide, // <-- 3. Default to native Oxide
        }
    }
}

// 2. The Shell struct lives on its own, and USES the state
pub struct Shell {
    pub state: ShellState,
}

impl Shell {
    pub fn new() -> Self {
        Self {
            state: ShellState::new(),
        }
    }
}