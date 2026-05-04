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
        let mut _last_exit = 0;
        for _stmt in statements {
            // In the next phase, we will map Statements to 
            // Executor actions here. For now, we clear the warnings.
        }
        _last_exit
    }
}