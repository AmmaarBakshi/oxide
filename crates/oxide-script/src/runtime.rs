use crate::scope::Scope;
use crate::functions::FunctionRegistry;
use crate::stdlib::StdLib;
use crate::modules::ModuleManager;
use oxide_parser::ast::Statement; // Fixes E0425, E0433

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
            match stmt {
                Statement::Command(cmd) => {
                    println!("DEBUG: Running command: {}", cmd.program);
                }
                Statement::Pipeline(cmds) => {
                    println!("DEBUG: Running pipeline with {} commands", cmds.len());
                }
                Statement::If { condition, body, else_if, else_body } => {
                    if self.eval_condition(condition) {
                        self.run_script(body);
                    } else {
                        let mut matched = false;
                        for (elif_cond, elif_body) in else_if {
                            if self.eval_condition(elif_cond) {
                                self.run_script(elif_body);
                                matched = true;
                                break;
                            }
                        }
                        if !matched {
                            if let Some(eb) = else_body {
                                self.run_script(eb);
                            }
                        }
                    }
                }
            }
        }
        last_exit
    }

    fn evaluate_condition(&self, condition: &str) -> bool {
        let val = if condition.starts_with('$') {
            self.scope.get(&condition[1..]).unwrap_or_else(|| "0".to_string())
        } else {
            condition.to_string()
        };
        !val.is_empty() && val != "0" && val != "false"
    }
}