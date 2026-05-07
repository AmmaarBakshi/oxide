use crate::scope::Scope;
use crate::functions::FunctionRegistry;
use crate::stdlib::StdLib;
use crate::modules::ModuleManager;
use oxide_parser::ast::{Statement, Command}; 

pub struct Runtime {
    pub scope: Scope,
    pub functions: FunctionRegistry,
    pub stdlib: StdLib,
    pub modules: ModuleManager,
}

impl Runtime {
    // 1. Added missing constructor (required by Executor)
    pub fn new() -> Self {
        Self {
            scope: Scope::new(),
            functions: FunctionRegistry::new(),
            stdlib: StdLib::new(),
            modules: ModuleManager::new(),
        }
    }

    pub fn run_script(&mut self, statements: Vec<Statement>) {
        for stmt in statements {
            match stmt {
                // 2. Fixed pattern matching to use 'cmd'
                Statement::Command(cmd) => {
                    println!("DEBUG: Running script command: {}", cmd.program);
                    self.execute_command(cmd);
                }
                Statement::If { condition, body, else_if, else_body } => {
                    if self.eval_condition(&condition) {
                        self.run_script(body);
                    } else {
                        let mut matched = false;
                        for (elif_cond, elif_body) in else_if {
                            if self.eval_condition(&elif_cond) {
                                self.run_script(elif_body);
                                matched = true;
                                break;
                            }
                        }
                        if !matched {
                            if let Some(eb) = else_body { self.run_script(eb); }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn eval_condition(&mut self, condition: &str) -> bool {
        let val = if condition.starts_with('$') {
            self.scope.get(&condition[1..]).unwrap_or_else(|| "0".to_string())
        } else {
            condition.to_string()
        };
        !val.is_empty() && val != "0" && val != "false"
    }

    // 3. Added the missing execution stub
    fn execute_command(&mut self, _cmd: Command) {
        // Scripts pass commands back up to the Executor in a full shell, 
        // but this stub satisfies the script engine compiler for now.
    }
}