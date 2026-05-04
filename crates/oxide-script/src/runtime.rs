use crate::scope::Scope;
use oxide_parser::ast::Statement;

pub struct Runtime {
    pub global_scope: Scope,
}

impl Runtime {
    pub fn new() -> Self {
        Self {
            global_scope: Scope::new(),
        }
    }

    pub fn run_script(&mut self, statements: Vec<Statement>) -> i32 {
        let mut last_exit = 0;
        for stmt in statements {
            // Here we will bridge to the executor logic
            // but with script-specific logic like loops/ifs
        }
        last_exit
    }
}