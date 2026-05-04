use crate::scope::Scope;
use crate::functions::FunctionRegistry;
use crate::stdlib::StdLib;
use crate::modules::ModuleManager;
use oxide_parser::ast::Statement; 


pub struct Runtime {
    pub scope: Scope,
    pub functions: FunctionRegistry,
    pub stdlib: StdLib,
    pub modules: ModuleManager,
}

impl Runtime {
    pub fn new() -> Self {
        Self {
            scope: Scope::new(),
            functions: FunctionRegistry::new(),
            stdlib: StdLib::new(),
            modules: ModuleManager::new(),
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