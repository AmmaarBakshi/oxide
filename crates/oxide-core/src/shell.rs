use std::collections::HashMap;

// 1. The State struct lives on its own
pub struct ShellState {
    pub is_running: bool,
    pub last_exit_code: i32,
    pub aliases: HashMap<String, String>,
}

impl ShellState {
    pub fn new() -> Self {
        Self {
            is_running: true,
            last_exit_code: 0,
            aliases: HashMap::new(),
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