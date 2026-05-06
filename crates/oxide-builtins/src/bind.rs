// crates/oxide-builtins/src/bind.rs
use std::collections::HashMap;

pub enum ShellAction {
    Complete,      // Tab
    HistorySearch, // Ctrl+R
    ClearScreen,   // Ctrl+L
    KillLine,      // Ctrl+U
    Custom(String), // Custom macros
}

pub struct BindManager {
    // Maps key sequences like "\C-l" to actions
    pub key_map: HashMap<String, ShellAction>,
}