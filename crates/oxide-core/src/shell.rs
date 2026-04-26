use crate::state::ShellState;

pub struct Shell {
    pub state: ShellState,
    // Later we will add: env, config, history, etc.
}

impl Shell {
    pub fn new() -> Self {
        Self {
            state: ShellState::new(),
        }
    }
}