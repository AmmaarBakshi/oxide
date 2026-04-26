pub struct ShellState {
    pub is_running: bool,
    pub last_exit_code: i32,
}

impl ShellState {
    pub fn new() -> Self {
        Self {
            is_running: true,
            last_exit_code: 0,
        }
    }
}